"""Minimal binary FBX (7.x) reader: parses the node tree and extracts skeleton, animation curves and blend shapes."""

import struct
import zlib

import numpy as np


class Node:
	__slots__ = ("name", "props", "children")

	def __init__(self, name, props, children):
		self.name = name
		self.props = props
		self.children = children

	def find(self, name):
		for child in self.children:
			if child.name == name:
				return child
		return None

	def find_all(self, name):
		return [child for child in self.children if child.name == name]

	def __repr__(self):
		return f"Node({self.name}, {self.props!r:.80}, {len(self.children)} children)"


_ARRAY_TYPES = {
	"f": ("<f", 4),
	"d": ("<d", 8),
	"l": ("<q", 8),
	"i": ("<i", 4),
	"b": ("<?", 1),
}


def _read_property(data, offset):
	kind = chr(data[offset])
	offset += 1
	if kind == "Y":
		return struct.unpack_from("<h", data, offset)[0], offset + 2
	if kind == "C":
		return bool(data[offset]), offset + 1
	if kind == "I":
		return struct.unpack_from("<i", data, offset)[0], offset + 4
	if kind == "F":
		return struct.unpack_from("<f", data, offset)[0], offset + 4
	if kind == "D":
		return struct.unpack_from("<d", data, offset)[0], offset + 8
	if kind == "L":
		return struct.unpack_from("<q", data, offset)[0], offset + 8
	if kind in "SR":
		length = struct.unpack_from("<I", data, offset)[0]
		raw = data[offset + 4:offset + 4 + length]
		if kind == "S":
			raw = raw.decode("utf-8", errors="replace")
		return raw, offset + 4 + length
	if kind in _ARRAY_TYPES:
		count, encoding, length = struct.unpack_from("<III", data, offset)
		offset += 12
		raw = data[offset:offset + length]
		if encoding == 1:
			raw = zlib.decompress(raw)
		fmt, _ = _ARRAY_TYPES[kind]
		return np.frombuffer(raw, dtype=np.dtype(fmt), count=count), offset + length
	raise ValueError(f"Unknown FBX property type {kind!r} at {offset}")


def _read_node(data, offset, wide):
	if wide:
		end, num_props, _ = struct.unpack_from("<QQQ", data, offset)
		offset += 24
	else:
		end, num_props, _ = struct.unpack_from("<III", data, offset)
		offset += 12
	name_len = data[offset]
	offset += 1
	if end == 0:
		return None, offset + name_len
	name = data[offset:offset + name_len].decode("ascii")
	offset += name_len
	props = []
	for _ in range(num_props):
		value, offset = _read_property(data, offset)
		props.append(value)
	children = []
	sentinel = 25 if wide else 13
	while offset < end:
		if end - offset == sentinel and not any(data[offset:end]):
			offset = end
			break
		child, offset = _read_node(data, offset, wide)
		if child is None:
			break
		children.append(child)
	return Node(name, props, children), end


def parse(path):
	with open(path, "rb") as file:
		data = file.read()
	if not data.startswith(b"Kaydara FBX Binary  \x00"):
		raise ValueError(f"{path}: not a binary FBX file")
	version = struct.unpack_from("<I", data, 23)[0]
	wide = version >= 7500
	offset = 27
	roots = []
	while offset < len(data):
		node, offset = _read_node(data, offset, wide)
		if node is None:
			break
		roots.append(node)
	return Node("Root", [], roots)


def properties70(node):
	"""Returns `{name: values}` from a node's `Properties70` block."""
	result = {}
	block = node.find("Properties70")
	if block is None:
		return result
	for p in block.find_all("P"):
		result[p.props[0]] = p.props[4:]
	return result


def object_name(raw):
	return raw.split("\x00\x01")[0]


FBX_TICKS = 46186158000


def euler_matrix(angles_deg, order=0):
	"""FBX euler rotation (degrees) to a 3x3 matrix. Order 0 = XYZ (X applied first)."""
	x, y, z = np.radians(angles_deg)
	cx, sx, cy, sy, cz, sz = np.cos(x), np.sin(x), np.cos(y), np.sin(y), np.cos(z), np.sin(z)
	rx = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
	ry = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
	rz = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
	mats = {"X": rx, "Y": ry, "Z": rz}
	orders = ["XYZ", "XZY", "YZX", "YXZ", "ZXY", "ZYX"]
	seq = orders[order]
	result = np.eye(3)
	for axis in seq:
		result = mats[axis] @ result
	return result


class Model:
	def __init__(self, uid, name, kind, props):
		self.uid = uid
		self.name = name
		self.kind = kind
		self.parent = None
		self.children = []
		self.translation = np.array(props.get("Lcl Translation", (0, 0, 0)), dtype=float)
		self.rotation = np.array(props.get("Lcl Rotation", (0, 0, 0)), dtype=float)
		self.scaling = np.array(props.get("Lcl Scaling", (1, 1, 1)), dtype=float)
		self.pre_rotation = np.array(props.get("PreRotation", (0, 0, 0)), dtype=float)
		self.post_rotation = np.array(props.get("PostRotation", (0, 0, 0)), dtype=float)
		self.rotation_order = int(props.get("RotationOrder", (0,))[0])
		# {"T"|"R"|"S": {axis index: (times seconds, values)}}
		self.curves = {}

	def local_matrix(self, t=None, rest=False):
		trans = self.translation.copy()
		rot = self.rotation.copy()
		scale = self.scaling.copy()
		if not rest and t is not None:
			for channel, target in (("T", trans), ("R", rot), ("S", scale)):
				for axis, (times, values) in self.curves.get(channel, {}).items():
					target[axis] = np.interp(t, times, values)
		mat = np.eye(4)
		r = euler_matrix(self.pre_rotation) @ euler_matrix(rot, self.rotation_order) @ euler_matrix(self.post_rotation).T
		mat[:3, :3] = r @ np.diag(scale)
		mat[:3, 3] = trans
		return mat


class Scene:
	def __init__(self, path):
		self.root = parse(path)
		self.models = {}
		self.blend_channels = {}  # uid -> name
		self.blend_curves = {}  # channel name -> (times, values)
		self.frame_rate = 30.0
		self.anim_span = None
		self._load()

	def _load(self):
		settings = self.root.find("GlobalSettings")
		if settings is not None:
			gp = properties70(settings)
			time_mode = gp.get("TimeMode", (0,))[0]
			rates = {1: 120, 2: 100, 3: 60, 4: 50, 5: 48, 6: 30, 7: 30, 8: 30, 9: 29.97, 10: 25, 11: 24, 13: 96, 14: 72, 15: 59.94}
			if time_mode == 14 or time_mode not in rates:
				self.frame_rate = float(gp.get("CustomFrameRate", (30.0,))[0])
				if self.frame_rate <= 0:
					self.frame_rate = 30.0
			else:
				self.frame_rate = float(rates[time_mode])
			self.up_axis = gp.get("UpAxis", (1,))[0]

		objects = self.root.find("Objects")
		curve_nodes = {}
		curves = {}
		for obj in objects.children:
			uid = obj.props[0]
			if obj.name == "Model":
				self.models[uid] = Model(uid, object_name(obj.props[1]), obj.props[2], properties70(obj))
			elif obj.name == "AnimationCurveNode":
				curve_nodes[uid] = object_name(obj.props[1])
			elif obj.name == "AnimationCurve":
				times = obj.find("KeyTime").props[0].astype(np.float64) / FBX_TICKS
				values = obj.find("KeyValueFloat").props[0].astype(np.float64)
				curves[uid] = (times, values)
			elif obj.name == "Deformer" and obj.props[2] == "BlendShapeChannel":
				self.blend_channels[uid] = object_name(obj.props[1])
			elif obj.name == "AnimationStack":
				sp = properties70(obj)
				if "LocalStart" in sp and "LocalStop" in sp:
					self.anim_span = (sp["LocalStart"][0] / FBX_TICKS, sp["LocalStop"][0] / FBX_TICKS)

		curve_target = {}  # curve node uid -> (model uid, channel)
		curve_axis = {}  # curve uid -> (curve node uid, axis)
		for c in self.root.find("Connections").find_all("C"):
			kind, child, parent = c.props[0], c.props[1], c.props[2]
			if kind == "OO":
				if child in self.models and parent in self.models:
					self.models[child].parent = self.models[parent]
					self.models[parent].children.append(self.models[child])
			elif kind == "OP":
				prop = c.props[3]
				if child in curve_nodes and parent in self.models:
					channel = {"Lcl Translation": "T", "Lcl Rotation": "R", "Lcl Scaling": "S"}.get(prop)
					if channel:
						curve_target[child] = (parent, channel)
				elif child in curve_nodes and parent in self.blend_channels and prop == "DeformPercent":
					curve_target[child] = (parent, "DeformPercent")
				elif child in curves and parent in curve_nodes:
					axis = {"d|X": 0, "d|Y": 1, "d|Z": 2, "d|DeformPercent": 0}.get(prop)
					if axis is not None:
						curve_axis[child] = (parent, axis)

		for curve_uid, (node_uid, axis) in curve_axis.items():
			if node_uid not in curve_target:
				continue
			target_uid, channel = curve_target[node_uid]
			if channel == "DeformPercent":
				self.blend_curves[self.blend_channels[target_uid]] = curves[curve_uid]
			else:
				self.models[target_uid].curves.setdefault(channel, {})[axis] = curves[curve_uid]

	def model(self, name):
		for model in self.models.values():
			if model.name == name:
				return model
		return None

	def global_matrices(self, t=None, rest=False):
		cache = {}

		def world(model):
			if model.uid in cache:
				return cache[model.uid]
			local = model.local_matrix(t, rest)
			mat = local if model.parent is None else world(model.parent) @ local
			cache[model.uid] = mat
			return mat

		return {model.name: world(model) for model in self.models.values()}

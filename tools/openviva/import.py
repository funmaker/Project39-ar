#!/usr/bin/env python3
"""Imports OpenViva's character ("Lily"/Shinobu) animations, animator controller, animation manifest and voice
into `assets/openviva`.

Usage: python3 -I tools/openviva/import.py <path to OpenViva project> [<output dir>] [--data-only]

`--data-only` skips re-baking the clips (slow) and only regenerates the JSON data files.

Animations are baked into a skeleton independent "humanoid" space: every bone's rest pose (the A-pose bind pose
of the source rig) has an identity rotation in a canonical frame (+X = character's left, +Y = up, +Z = forward),
so the clips can be retargeted at runtime to any model (see `src/component/miku/anim/`).
"""

import json
import os
import re
import struct
import subprocess
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import fbx  # noqa: E402
import manifest  # noqa: E402
import unity  # noqa: E402

# Canonical humanoid bones: (name, parent, source bone, source tip used to compute the rest direction)
FINGERS = [("Thumb", "thumb"), ("Index", "index"), ("Middle", "middle"), ("Ring", "ring"), ("Little", "pinky")]


def _humanoid():
	bones = [
		("Hips", None, "spine1", "spine2"),
		("Spine", "Hips", "spine2", "spine3"),
		("Chest", "Spine", "spine3", "neck"),
		("Neck", "Chest", "neck", "head"),
		("Head", "Neck", "head", None),
	]
	for side, s in (("Left", "l"), ("Right", "r")):
		bones += [
			(f"{side}Eye", "Head", f"eyeball_{s}", None),
			(f"{side}Shoulder", "Chest", f"shoulder_{s}", f"armControl_{s}"),
			(f"{side}UpperArm", f"{side}Shoulder", f"armControl_{s}", f"forearmControl_{s}"),
			(f"{side}LowerArm", f"{side}UpperArm", f"forearmControl_{s}", f"hand_{s}"),
			(f"{side}Hand", f"{side}LowerArm", f"hand_{s}", f"middle1_{s}"),
		]
		for finger, src in FINGERS:
			bones += [
				(f"{side}{finger}Proximal", f"{side}Hand", f"{src}1_{s}", f"{src}2_{s}"),
				(f"{side}{finger}Intermediate", f"{side}{finger}Proximal", f"{src}2_{s}", f"{src}3_{s}"),
				(f"{side}{finger}Distal", f"{side}{finger}Intermediate", f"{src}3_{s}", f"{src}3_{s}_end"),
			]
		bones += [
			(f"{side}UpperLeg", "Hips", f"upperLegControl_{s}", f"leg_{s}"),
			(f"{side}LowerLeg", f"{side}UpperLeg", f"leg_{s}", f"foot_{s}"),
			(f"{side}Foot", f"{side}LowerLeg", f"foot_{s}", f"toeIndex1_{s}"),
			(f"{side}Toes", f"{side}Foot", f"toeIndex1_{s}", f"toeIndex2_{s}"),
		]
	return bones


HUMANOID = _humanoid()
BONE_NAMES = [b[0] for b in HUMANOID]
BONE_INDEX = {name: i for i, name in enumerate(BONE_NAMES)}

# Animations used by the port: everything the character can do while standing, hugging, sitting, squatting or
# ragdolling, without the ones that need props or places (bath, bed, cooking, camera, bag, horse, cards, ...).
# Kissing and hugging are player interactions, so they are kept.
INCLUDED_STATES = {"STAND", "STANDING_HUG", "FLOOR_SIT", "SQUAT", "OFFBALANCE"}
EXCLUDED = re.compile(r"BATHTUB|CATTAIL|EGG|MIXING|MORTAR|BAG|DONUT|EAT_ITEM|SMELL_ITEM|PHOTO|POLAROID|HORSEBACK|SLIDINGDOOR|"
                      r"SUNHAT|WHEAT|CARD|BEG|MERCHANT|JEALOUS|THROW|TIP_TOE|PICKUP|CHANGE_ITEM|TO_BED|SPLASHED|"
                      r"CHASE|REACT_PERV|CRAWL|RELAX|POINT_OUT|OUTFIT_LIKE|POSE_PEACE|SOCIAL|STEAL|SCARED")


# ---------------------------------------------------------------------------------------------------------------------
# Math

def quat_from_matrix(m):
	t = np.trace(m)
	if t > 0:
		s = np.sqrt(t + 1.0) * 2
		w, x, y, z = 0.25 * s, (m[2, 1] - m[1, 2]) / s, (m[0, 2] - m[2, 0]) / s, (m[1, 0] - m[0, 1]) / s
	elif m[0, 0] > m[1, 1] and m[0, 0] > m[2, 2]:
		s = np.sqrt(1.0 + m[0, 0] - m[1, 1] - m[2, 2]) * 2
		w, x, y, z = (m[2, 1] - m[1, 2]) / s, 0.25 * s, (m[0, 1] + m[1, 0]) / s, (m[0, 2] + m[2, 0]) / s
	elif m[1, 1] > m[2, 2]:
		s = np.sqrt(1.0 + m[1, 1] - m[0, 0] - m[2, 2]) * 2
		w, x, y, z = (m[0, 2] - m[2, 0]) / s, (m[0, 1] + m[1, 0]) / s, 0.25 * s, (m[1, 2] + m[2, 1]) / s
	else:
		s = np.sqrt(1.0 + m[2, 2] - m[0, 0] - m[1, 1]) * 2
		w, x, y, z = (m[1, 0] - m[0, 1]) / s, (m[0, 2] + m[2, 0]) / s, (m[1, 2] + m[2, 1]) / s, 0.25 * s
	q = np.array([x, y, z, w])
	return q / np.linalg.norm(q)


def orthonormalize(m):
	u, _, vt = np.linalg.svd(m)
	r = u @ vt
	if np.linalg.det(r) < 0:
		u[:, -1] *= -1
		r = u @ vt
	return r


# ---------------------------------------------------------------------------------------------------------------------
# Skeleton

class Skeleton:
	"""Bind pose of the source rig and the canonical frame."""

	def __init__(self, scene):
		self.bind = {}
		for obj in scene.root.find("Objects").children:
			if obj.name == "Pose":
				for pose_node in obj.find_all("PoseNode"):
					uid = pose_node.find("Node").props[0]
					if uid in scene.models:
						self.bind[scene.models[uid].name] = pose_node.find("Matrix").props[0].reshape(4, 4).T.astype(float)
		# Bones missing from the bind pose (end bones) are placed using their rest local transform.
		for model in scene.models.values():
			self._bind_of(model)

		pos = lambda name: self.bind[name][:3, 3]
		up = pos("head") - pos("spine1")
		up /= np.linalg.norm(up)
		left = pos("armControl_l") - pos("armControl_r")
		left -= up * np.dot(left, up)
		left /= np.linalg.norm(left)
		forward = np.cross(left, up)
		self.frame = np.stack([left, up, forward])  # rows: canonical axes in FBX space
		self.hips_height = float(np.dot(pos("spine1") - self.bind["ShinobuArmature"][:3, 3], up))

	def _bind_of(self, model):
		if model.name in self.bind:
			return self.bind[model.name]
		if model.parent is None:
			mat = model.local_matrix(rest=True)
		else:
			mat = self._bind_of(model.parent) @ model.local_matrix(rest=True)
		self.bind[model.name] = mat
		return mat

	def rest_rotation(self, name):
		return orthonormalize(self.bind[name][:3, :3])

	def to_json(self):
		bones = []
		for name, parent, src, tip in HUMANOID:
			entry = {"name": name, "parent": parent}
			if tip is not None:
				direction = self.frame @ (self.bind[tip][:3, 3] - self.bind[src][:3, 3])
				entry["direction"] = list(np.round(direction / np.linalg.norm(direction), 5))
			position = self.frame @ (self.bind[src][:3, 3] - self.bind["ShinobuArmature"][:3, 3]) / self.hips_height
			entry["position"] = list(np.round(position, 5))
			bones.append(entry)
		return {"bones": bones}


# ---------------------------------------------------------------------------------------------------------------------
# Clips

def mirror_bone_name(name):
	if name.startswith("Left"):
		return "Right" + name[4:]
	if name.startswith("Right"):
		return "Left" + name[5:]
	return name


def mirror_morph_name(name):
	if name.endswith("_l"):
		return name[:-2] + "_r"
	if name.endswith("_r"):
		return name[:-2] + "_l"
	return name


def bake_clip(scene, skeleton, first_frame, last_frame, mirror):
	"""Samples a clip into (local rotations [frames, bones, 4], root translation [frames, 3], morphs {name: [frames]})."""
	frame = skeleton.frame
	rest = {src: skeleton.rest_rotation(src) for _, _, src, _ in HUMANOID}
	hips_rest = skeleton.bind["spine1"][:3, 3]
	frames = list(range(int(first_frame), int(last_frame) + 1))
	rotations = np.zeros((len(frames), len(HUMANOID), 4))
	translations = np.zeros((len(frames), 3))
	for fi, f in enumerate(frames):
		t = f / scene.frame_rate
		world = scene.global_matrices(t)
		deltas = {}
		for name, parent, src, _ in HUMANOID:
			r = orthonormalize(world[src][:3, :3])
			deltas[name] = frame @ (r @ rest[src].T) @ frame.T
		for bi, (name, parent, src, _) in enumerate(HUMANOID):
			local = deltas[name] if parent is None else deltas[parent].T @ deltas[name]
			rotations[fi, bi] = quat_from_matrix(local)
		translations[fi] = frame @ (world["spine1"][:3, 3] - hips_rest) / skeleton.hips_height

	morphs = {}
	for name, (times, values) in scene.blend_curves.items():
		morphs[name] = np.clip(np.interp(np.array(frames) / scene.frame_rate, times, values) / 100.0, 0.0, 1.0)

	if mirror:
		mirrored = np.empty_like(rotations)
		for bi, name in enumerate(BONE_NAMES):
			q = rotations[:, BONE_INDEX[mirror_bone_name(name)]]
			mirrored[:, bi] = np.stack([q[:, 0], -q[:, 1], -q[:, 2], q[:, 3]], axis=1)
		rotations = mirrored
		translations = translations * np.array([-1.0, 1.0, 1.0])
		morphs = {mirror_morph_name(name): values for name, values in morphs.items()}

	# Keep quaternions in a continuous hemisphere so that linear interpolation between frames is sane.
	for fi in range(1, len(frames)):
		dots = np.sum(rotations[fi] * rotations[fi - 1], axis=1)
		rotations[fi][dots < 0] *= -1
	return rotations, translations, morphs


def write_clip(path, frame_rate, looping, rotations, translations, morphs):
	morph_names = sorted(morphs.keys())
	with open(path, "wb") as file:
		file.write(b"P39A")
		file.write(struct.pack("<IfIB", 1, frame_rate, len(rotations), int(looping)))
		file.write(struct.pack("<H", len(BONE_NAMES)))
		for name in BONE_NAMES:
			file.write(struct.pack("<B", len(name)) + name.encode())
		file.write(struct.pack("<H", len(morph_names)))
		for name in morph_names:
			file.write(struct.pack("<B", len(name)) + name.encode())
		quantized = np.clip(np.round(rotations * 32767), -32767, 32767).astype("<i2")
		for fi in range(len(rotations)):
			file.write(quantized[fi].tobytes())
			file.write(translations[fi].astype("<f4").tobytes())
			file.write(bytes(int(round(morphs[name][fi] * 255)) for name in morph_names))


# ---------------------------------------------------------------------------------------------------------------------
# Animator controller

class ControllerExporter:
	def __init__(self, controller_path, guids):
		self.docs = unity.load_documents(controller_path)
		self.guids = guids
		self.clips = {}  # clip key -> (fbx path, clip definition)
		self.fbx_cache = {}

	def layers(self):
		controller = next(d["AnimatorController"] for c, d in self.docs.values() if c == 91)
		return controller["m_AnimatorLayers"]

	def states(self, layer):
		machine_id = layer["m_StateMachine"]["fileID"]
		result = {}
		stack = [machine_id]
		while stack:
			machine = self.docs[stack.pop()][1]["AnimatorStateMachine"]
			for child in machine.get("m_ChildStates") or []:
				state = self.docs[child["m_State"]["fileID"]][1]["AnimatorState"]
				result[state["m_Name"]] = state
			for child in machine.get("m_ChildStateMachines") or []:
				stack.append(child["m_StateMachine"]["fileID"])
		return result

	def _fbx_clip(self, ref):
		guid = ref.get("guid")
		path = self.guids.path(guid)
		if path is None or not path.lower().endswith(".fbx"):
			return None
		if path not in self.fbx_cache:
			self.fbx_cache[path] = unity.fbx_clips(path)
		ids, definitions = self.fbx_cache[path]
		name = ids.get(int(ref["fileID"]))
		if name is None:
			return None
		definition = next((d for d in definitions if d["name"] == name), None)
		stem = os.path.splitext(os.path.basename(path))[0]
		key = name if name == stem else f"{stem}@{name}"
		self.clips[key] = (path, name, definition)
		return key

	def motion(self, ref):
		if ref is None or int(ref.get("fileID", 0)) == 0:
			return None
		if "guid" in ref:
			clip = self._fbx_clip(ref)
			return {"clip": clip} if clip else None
		class_id, data = self.docs[int(ref["fileID"])]
		if class_id != 206:
			return None
		tree = data["BlendTree"]
		children = []
		for child in tree.get("m_Childs") or []:
			motion = self.motion(child.get("m_Motion"))
			if motion is None:
				continue
			children.append({
				"motion": motion,
				"threshold": float(child.get("m_Threshold", 0)),
				"position": [float(child["m_Position"]["x"]), float(child["m_Position"]["y"])],
				"time_scale": float(child.get("m_TimeScale", 1)),
				"mirror": bool(child.get("m_Mirror", 0)),
				"direct_parameter": child.get("m_DirectBlendParameter") or "",
			})
		return {"tree": {
			"blend_type": int(tree.get("m_BlendType", 0)),
			"parameter": tree.get("m_BlendParameter") or "",
			"parameter_y": tree.get("m_BlendParameterY") or "",
			"children": children,
		}}

	def state(self, state):
		return {
			"motion": self.motion(state.get("m_Motion")),
			"speed": float(state.get("m_Speed", 1)),
			"speed_parameter": state.get("m_SpeedParameter") if state.get("m_SpeedParameterActive") else None,
			"time_parameter": state.get("m_TimeParameter") if state.get("m_TimeParameterActive") else None,
			"mirror": bool(state.get("m_Mirror", 0)),
			"cycle_offset": float(state.get("m_CycleOffset", 0)),
		}


# ---------------------------------------------------------------------------------------------------------------------
# Voice

def export_voice(viva, guids, out_dir, voice_lines):
	voice_asset = unity.load_documents(os.path.join(viva, "Assets/ScriptableObjects/Sound Sets/voices/shinobu_voice.asset"))
	voice = next(iter(voice_asset.values()))[1]["MonoBehaviour"]
	sound_dir = os.path.join(out_dir, "voice")
	os.makedirs(sound_dir, exist_ok=True)
	result = {}
	for line, ref in zip(voice_lines, voice["voiceLines"]):
		path = guids.path(ref["guid"])
		if path is None:
			continue
		sound_set = next(iter(unity.load_documents(path).values()))[1]["MonoBehaviour"]
		files = []
		for sound in sound_set.get("sounds") or []:
			clip_path = guids.path(sound["guid"]) if sound and "guid" in sound else None
			if clip_path is None:
				continue
			name = os.path.splitext(os.path.basename(clip_path))[0] + ".ogg"
			target = os.path.join(sound_dir, name)
			if not os.path.exists(target):
				subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", clip_path, "-ac", "1", "-c:a", "libvorbis", "-q:a", "3", target], check=True)
			files.append("openviva/voice/" + name)
		result[line] = files
	return result


def export_audio(guids, out_dir, guid, name):
	"""Converts a single audio clip asset into an ogg file and returns its path."""
	path = guids.path(guid)
	return export_sound_file(path, out_dir, name + ".ogg")


def export_sound_file(path, out_dir, file_name):
	sound_dir = os.path.join(out_dir, "sfx")
	os.makedirs(sound_dir, exist_ok=True)
	target = os.path.join(sound_dir, file_name)
	if not os.path.exists(target):
		subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", path, "-ac", "1", "-c:a", "libvorbis", "-q:a", "3", target], check=True)
	return "openviva/sfx/" + file_name


def export_sound_set(guids, out_dir, guid):
	"""Converts a `SoundSet` asset (e.g. impact or get-up sounds) into a list of ogg files."""
	path = guids.path(guid)
	sound_set = next(iter(unity.load_documents(path).values()))[1]["MonoBehaviour"]
	sound_dir = os.path.join(out_dir, "sfx")
	os.makedirs(sound_dir, exist_ok=True)
	files = []
	for sound in sound_set.get("sounds") or []:
		clip_path = guids.path(sound["guid"]) if sound and "guid" in sound else None
		if clip_path is None:
			continue
		file_name = os.path.splitext(os.path.basename(clip_path))[0] + ".ogg"
		target = os.path.join(sound_dir, file_name)
		if not os.path.exists(target):
			subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", clip_path, "-ac", "1", "-c:a", "libvorbis", "-q:a", "3", target], check=True)
		files.append("openviva/sfx/" + file_name)
	return files


def manifest_enum(viva, name):
	with open(os.path.join(viva, "Assets/Scripts/Lily/Lily.Manifest.cs"), encoding="utf-8-sig") as file:
		text = file.read()
	body = re.search(r"enum " + name + r"\s*\{(.*?)\}", text, flags=re.S).group(1)
	body = re.sub(r"//[^\n]*", "", body)
	return [v.split("=")[0].strip() for v in body.split(",") if v.strip()]


def voice_line_enum(viva):
	return manifest_enum(viva, "VoiceLine")


# ---------------------------------------------------------------------------------------------------------------------

def main():
	args = [a for a in sys.argv[1:] if not a.startswith("--")]
	viva = args[0]
	out_dir = args[1] if len(args) > 1 else os.path.join(os.path.dirname(__file__), "../../assets/openviva")
	assets = os.path.join(viva, "Assets")
	lily = os.path.join(assets, "Scripts/Lily")
	os.makedirs(os.path.join(out_dir, "anims"), exist_ok=True)

	print("Indexing GUIDs...")
	guids = unity.GuidIndex(assets)

	print("Parsing manifest...")
	animations = manifest.parse_manifest(os.path.join(lily, "Lily.AnimationManifest.cs"))
	body_states = manifest.parse_body_states(os.path.join(lily, "Lily.BodyStatesAnimationSets.cs"))
	selected = {name: info for name, info in animations.items()
	            if name == "NONE" or (info["condition"] in INCLUDED_STATES and info["new_state"] in INCLUDED_STATES and not EXCLUDED.search(name))}
	for info in selected.values():
		if info["transition"]["kind"] == "next" and info["transition"]["next"] not in selected:
			info["transition"] = {"kind": "idle"}
	for state, data in body_states.items():
		for set_name, anims in list(data["sets"].items()):
			data["sets"][set_name] = [a for a in anims if a in selected]
		data["connections"] = {k: v for k, v in data["connections"].items() if v in selected}
	body_states = {k: v for k, v in body_states.items() if k in INCLUDED_STATES or k == "NONE"}

	print("Parsing animator controller...")
	controller = ControllerExporter(os.path.join(assets, "Resources/Animations/LilyAC.controller"), guids)
	layers_out = []
	layer_keys = ["legs", "torso", "face"]
	for layer in controller.layers():
		if layer["m_Name"] not in layer_keys:
			continue
		key = {"legs": "legs", "torso": "torso", "face": "face"}[layer["m_Name"]]
		states = controller.states(layer)
		needed = {info[key] for info in selected.values()}
		needed.add("APOSE")
		out_states = {}
		for name in sorted(needed):
			if name in states:
				exported = controller.state(states[name])
				if exported["motion"] is not None:
					out_states[name] = exported
			else:
				print(f"  warning: state {name} missing in layer {layer['m_Name']}")
		layers_out.append({"name": layer["m_Name"], "states": out_states})

	parameters = [p["m_Name"] for p in next(d["AnimatorController"] for c, d in controller.docs.values() if c == 91)["m_AnimatorParameters"]]

	data_only = "--data-only" in sys.argv
	print(f"Baking {len(controller.clips)} clips...")
	reference = fbx.Scene(os.path.join(assets, "Resources/Animations/shinobu/BUILD/blender_exports/APOSE.fbx"))
	skeleton = Skeleton(reference)
	clips_out = {}
	for i, (key, (path, take, definition)) in enumerate(sorted(controller.clips.items())):
		if data_only:
			with open(os.path.join(out_dir, "controller.json")) as file:
				clips_out = json.load(file)["clips"]
			break
		scene = fbx.Scene(path)
		if definition is not None:
			first, last = definition["firstFrame"], definition["lastFrame"]
			mirror = bool(definition.get("mirror", 0))
			looping = bool(definition.get("loopTime", 0))
		else:
			first = round(scene.anim_span[0] * scene.frame_rate)
			last = round(scene.anim_span[1] * scene.frame_rate)
			mirror, looping = False, False
		rotations, translations, morphs = bake_clip(scene, skeleton, first, last, mirror)
		file_name = re.sub(r"[^\w@.-]", "_", key) + ".p39a"
		write_clip(os.path.join(out_dir, "anims", file_name), scene.frame_rate, looping, rotations, translations, morphs)
		clips_out[key] = {"file": "openviva/anims/" + file_name, "length": (last - first) / scene.frame_rate, "looping": looping}
		print(f"  [{i + 1}/{len(controller.clips)}] {key}: {last - first + 1} frames{' (mirrored)' if mirror else ''}")

	print("Converting voice...")
	voice = export_voice(viva, guids, out_dir, voice_line_enum(viva))
	enum_order = [name for name in manifest_enum(viva, "Animation")]
	settings = next(iter(unity.load_documents(os.path.join(assets, "ScriptableObjects/LilySettings.asset")).values()))[1]["MonoBehaviour"]
	sfx = {
		"get_up": export_sound_set(guids, out_dir, settings["m_getUpSound"]["guid"]),
		"impact_soft": export_sound_set(guids, out_dir, settings["m_bodyImpactSoftSound"]["guid"]),
		"impact_hard": export_sound_set(guids, out_dir, settings["m_bodyImpactHardSound"]["guid"]),
		# Player gesture confirmation sounds from `ObjectFingerPointer` in PLAYER.prefab.
		"gesture_follow": [export_audio(guids, out_dir, "3a5aafad270dc2a6c8ee3006dfb3623a", "gesture_follow")],
		"gesture_present": [export_audio(guids, out_dir, "a17d2d760ad58888fb1ca15e3cee23a2", "gesture_present")],
		"gesture_hello": [export_audio(guids, out_dir, "4d92c0bf962bac82b81243623097fd65", "gesture_hello")],
		"gesture_mechanism": [export_audio(guids, out_dir, "f92703966e1a730cd9ba44579ec31adc", "gesture_mechanism")],
	}

	with open(os.path.join(out_dir, "skeleton.json"), "w") as file:
		json.dump(skeleton.to_json(), file, indent="\t")
	with open(os.path.join(out_dir, "controller.json"), "w") as file:
		json.dump({"parameters": parameters, "layers": layers_out, "clips": clips_out}, file, indent="\t")
	with open(os.path.join(out_dir, "animations.json"), "w") as file:
		json.dump({"animations": selected, "body_states": body_states, "enum_order": enum_order}, file, indent="\t")
	with open(os.path.join(out_dir, "voice.json"), "w") as file:
		json.dump({"lines": voice, "sfx": sfx}, file, indent="\t")
	print("Done.")


if __name__ == "__main__":
	main()

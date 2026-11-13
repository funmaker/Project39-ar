"""Helpers for reading Unity YAML assets (animator controllers, metas, scriptable objects)."""

import os
import re

import yaml


class _Loader(yaml.SafeLoader):
	pass


# Unity tags like `!u!114` are irrelevant for us; treat them as plain mappings.
_Loader.add_multi_constructor("tag:unity3d.com,2011:", lambda loader, suffix, node: loader.construct_mapping(node, deep=True))


def load_documents(path):
	"""Returns `{file_id: (class_id, body)}` for every document of a Unity YAML file."""
	with open(path, encoding="utf-8") as file:
		text = file.read()
	documents = {}
	parts = re.split(r"^--- !u!(\d+) &(-?\d+).*$", text, flags=re.M)
	for i in range(1, len(parts), 3):
		class_id, file_id, body = int(parts[i]), int(parts[i + 1]), parts[i + 2]
		data = yaml.load(body, Loader=_Loader)
		documents[file_id] = (class_id, data)
	return documents


class GuidIndex:
	"""Maps asset GUIDs to paths by scanning `.meta` files."""

	def __init__(self, root):
		self.paths = {}
		for dirpath, _, files in os.walk(root):
			for name in files:
				if not name.endswith(".meta"):
					continue
				path = os.path.join(dirpath, name)
				with open(path, encoding="utf-8", errors="replace") as file:
					for line in file:
						if line.startswith("guid: "):
							self.paths[line[6:].strip()] = path[:-5]
							break

	def path(self, guid):
		return self.paths.get(guid)


def load_meta(path):
	with open(path + ".meta", encoding="utf-8") as file:
		return yaml.load(file, Loader=_Loader)


def fbx_clips(fbx_path):
	"""Returns `({file_id: clip_name}, [clip definitions])` from a model's meta file."""
	meta = load_meta(fbx_path)
	importer = meta.get("ModelImporter", {})
	ids = {}
	for entry in importer.get("internalIDToNameTable") or []:
		first = entry.get("first", {})
		if 74 in first:
			ids[int(first[74])] = entry["second"]
	for key, name in (importer.get("fileIDToRecycleName") or {}).items():
		if 7400000 <= int(key) < 7500000:
			ids[int(key)] = name
	clips = (importer.get("animations") or {}).get("clipAnimations") or []
	for clip in clips:
		if "internalID" in clip and int(clip["internalID"]) != 0:
			ids.setdefault(int(clip["internalID"]), clip["name"])
	return ids, clips

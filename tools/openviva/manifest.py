"""Parses OpenViva's `Lily.AnimationManifest.cs` and `Lily.BodyStatesAnimationSets.cs` (C# source) into plain data."""

import re


def _balanced(text, start):
	"""Returns the index just after the parenthesis matching the one at `start`."""
	depth = 0
	i = start
	while i < len(text):
		c = text[i]
		if c == '"':
			i = text.index('"', i + 1)
		elif c == "(":
			depth += 1
		elif c == ")":
			depth -= 1
			if depth == 0:
				return i + 1
		i += 1
	raise ValueError("unbalanced")


def _split_args(text):
	"""Splits a comma separated argument list on the top level."""
	args, depth, current, i = [], 0, "", 0
	while i < len(text):
		c = text[i]
		if c == '"':
			end = text.index('"', i + 1)
			current += text[i:end + 1]
			i = end + 1
			continue
		if c in "([{":
			depth += 1
		elif c in ")]}":
			depth -= 1
		if c == "," and depth == 0:
			args.append(current.strip())
			current = ""
		else:
			current += c
		i += 1
	if current.strip():
		args.append(current.strip())
	return args


def _strip_comments(text):
	text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
	return re.sub(r"//[^\n]*", "", text)


def _enum_name(expr):
	return expr.split(".")[-1].strip()


def _mirror_name(name):
	return name.replace("_right", "_left")


def _mirror_enum(name):
	return name[:-len("_RIGHT")] + "_LEFT" if name.endswith("_RIGHT") else name


def _mirror_body_state(state):
	if "_RIGHT" in state:
		return state.replace("_RIGHT", "_LEFT")
	if "_LEFT" in state:
		return state.replace("_LEFT", "_RIGHT")
	return state


def _parse_events(expr):
	events = []
	if not expr or expr == "null":
		return events
	for match in re.finditer(r"new\s+LilyAnimationEvent\s*\(", expr):
		start = match.end() - 1
		end = _balanced(expr, start)
		args = _split_args(expr[start + 1:end - 1])
		time = float(args[0].rstrip("f"))
		name = re.search(r"AnimationEventName\.(\w+)", args[1]).group(1)
		values = []
		if len(args) > 2:
			inner = re.search(r"\{(.*)\}", args[2], flags=re.S)
			if inner:
				for value in _split_args(inner.group(1)):
					enum = re.search(r"VoiceLine\.(\w+)", value)
					if enum:
						values.append(enum.group(1))
						continue
					try:
						values.append(float(value.replace("(float)", "").replace("(int)", "").strip().rstrip("f")))
					except ValueError:
						values.append(value.split(".")[-1].strip())
		events.append({"time": time, "name": name, "values": values})
	return events


def _parse_logic(expr):
	match = re.search(r"AnimLogicInfo\s*\((.*)\)", expr, flags=re.S)
	args = _split_args(match.group(1)) if match else []
	if not args:
		return {"body_flags": 3, "body_flag_percent": 1.0, "body_flag_duration": 1.0, "eye_flags": 7, "eye_flag_duration": 1.0}
	values = [float(a.rstrip("f")) for a in args]
	return {
		"body_flags": int(values[0]),
		"body_flag_percent": values[1],
		"body_flag_duration": values[2],
		"eye_flags": int(values[3]),
		"eye_flag_duration": values[4],
	}


def _parse_transition(expr):
	expr = expr.strip()
	if expr == "idleTransition" or "TransitionToIdle" in expr:
		return {"kind": "idle"}
	match = re.search(r"DefaultTransition\s*\(\s*Lily\.Animation\.(\w+)", expr)
	if match:
		return {"kind": "next", "next": match.group(1)}
	if expr == "null":
		return {"kind": "loop"}
	match = re.search(r"new\s+(\w+\.)?(\w+)\s*\(", expr)
	return {"kind": "custom", "handler": match.group(2) if match else expr}


def parse_manifest(path):
	with open(path, encoding="utf-8-sig") as file:
		text = _strip_comments(file.read())
	animations = {}
	for match in re.finditer(r"RegisterAnimation\s*\(", text):
		start = match.end() - 1
		end = _balanced(text, start)
		args = _split_args(text[start + 1:end - 1])
		name = _enum_name(args[0])
		mirror = len(args) > 2 and args[2].strip() == "true"
		info = args[1]
		if "AnimationInfo(" not in info.replace(" ", ""):
			continue
		inner_start = info.index("(")
		inner = _split_args(info[inner_start + 1:_balanced(info, inner_start) - 1])
		torso, legs, face = (a.strip('"') for a in inner[1:4])
		entry = {
			"transition": _parse_transition(inner[0]),
			"torso": torso,
			"legs": legs,
			"face": face,
			"priority": _enum_name(inner[4]),
			"condition": _enum_name(inner[5]),
			"new_state": _enum_name(inner[6]),
			"transition_time": float(inner[7].rstrip("f")),
			"logic": _parse_logic(inner[8]),
			"events": _parse_events(inner[9] if len(inner) > 9 else None),
			"flags": [],
		}
		if len(inner) > 10:
			for flag in ("IDLE_STATE", "DISABLE_RAGDOLL_CHECK"):
				if flag in inner[10]:
					entry["flags"].append(flag)
		animations[name] = entry

		if mirror and name.endswith("_RIGHT"):
			left = dict(entry)
			transition = dict(entry["transition"])
			if transition["kind"] == "next":
				transition["next"] = _mirror_enum(transition["next"])
			left["transition"] = transition
			left["torso"] = _mirror_name(torso)
			left["legs"] = _mirror_name(legs)
			left["face"] = _mirror_name(face)
			left["condition"] = _mirror_body_state(entry["condition"])
			left["new_state"] = _mirror_body_state(entry["new_state"])
			left["events"] = [dict(e, name=e["name"].replace("_RIGHT", "_LEFT") if "_RIGHT" in e["name"] else e["name"]) for e in entry["events"]]
			animations[_mirror_enum(name)] = left

	for match in re.finditer(r"animationInfos\[Lily\.Animation\.(\w+)\]\.setTorsoSyncFloatID\(Animator\.StringToHash\(\"(\w+)\"\)\)", text):
		if match.group(1) in animations:
			animations[match.group(1)]["torso_sync"] = match.group(2)
	return animations


def parse_body_states(path):
	with open(path, encoding="utf-8-sig") as file:
		text = _strip_comments(file.read())
	variables = {}
	states = {}
	for match in re.finditer(r"var\s+(\w+)\s*=\s*new\s+BodyStateAnimationSet\s*\(\s*(true|false)\s*\)\s*;\s*bodyStateAnimationSets\[\(int\)BodyState\.(\w+)\]", text):
		variables[match.group(1)] = match.group(3)
		states[match.group(3)] = {"check_balance": match.group(2) == "true", "sets": {}, "connections": {}, "properties": {}}
	for match in re.finditer(r"(\w+)\.AddAnimation\(\s*AnimationSet\.(\w+)\s*,\s*Lily\.Animation\.(\w+)\s*\)", text):
		if match.group(1) in variables:
			states[variables[match.group(1)]]["sets"].setdefault(match.group(2), []).append(match.group(3))
	for match in re.finditer(r"(\w+)\.bodyStateConnections\[BodyState\.(\w+)\]\s*=\s*(?:Lily\.)?Animation\.(\w+)", text):
		if match.group(1) in variables:
			states[variables[match.group(1)]]["connections"][match.group(2)] = match.group(3)
	for match in re.finditer(r"(\w+)\.propertyValues\[PropertyValue\.(\w+)\]\s*=\s*([\d.]+)f?", text):
		if match.group(1) in variables:
			states[variables[match.group(1)]]["properties"][match.group(2)] = float(match.group(3))
	return states

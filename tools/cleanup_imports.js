const path = require("path");
const fs = require("fs");
const os = require("os");

const SRC_FILES = path.resolve(__dirname, "../src");

function getFiles(dir) {
	const subdirs = fs.readdirSync(dir);
	const files = subdirs.map((subdir) => {
		const res = path.resolve(dir, subdir);
		return (fs.statSync(res)).isDirectory() ? getFiles(res) : res;
	});
	return files.reduce((a, f) => a.concat(f), []);
}

const files = getFiles(SRC_FILES).filter(file => file.endsWith(".rs"));
const EOL = os.EOL;

for(const file of files) {
	const content = fs.readFileSync(file, "utf-8");
	const lines = content.split(/\r?\n/);
	const modules = {};
	const imports = {};
	const fileMeta = [];
	const externCrates = [];
	let parsed = 0;
	
	for(let line of lines) {
		const fullLine = line;
		line = line.trim();
		let prop = {
			meta: "",
			pub: false,
			macroUse: line.includes("#[macro_use]"),
		};
		
		if(line.startsWith("#[") && line.endsWith(";")) {
			prop.meta = line.slice(0, line.lastIndexOf("]") + 1);
			line = line.slice(prop.meta.length).trim();
		}
		
		prop.pub = line.startsWith("pub");
		
		let match;
		if(line.includes("extern crate")) {
			externCrates.push(fullLine);
		} else if(line.startsWith("#![")) {
			fileMeta.push(line);
		} else if((match = line.match(/^(?:pub )?use (.*)?;$/))) {
			parseImport(match[1], imports, prop)
		} else if((match = line.match(/^(?:pub )?mod (.*)?;$/))) {
			modules[match[1]] = prop;
		} else if(line.length !== 0) {
			break;
		}
		
		parsed += 1;
	}
	
	const selfPath = file.slice(SRC_FILES.length + 1)
											 .split(path.sep)
											 .filter(p => p !== "mod.rs");
	
	normalizeImports(imports, Object.keys(modules), selfPath);
	
	const importLines = generateImports(imports, [], Object.keys(modules));
	importLines.sort(compareLines);
	
	const modulesLines = generateModules(modules);
	modulesLines.sort(compareModules);
	
	let output = "";
	
	const externalImports = importLines.filter(line => !line.internal);
	const internalImports = importLines.filter(line => line.internal);
	
	if(fileMeta.length > 0) output += fileMeta.join(EOL) + EOL + EOL;
	if(externCrates.length > 0) output += externCrates.join(EOL) + EOL + EOL;
	if(externalImports.length > 0) output += externalImports.map(line => line.rendered).join(EOL) + EOL + EOL;
	if(modulesLines.length > 0) output += modulesLines.map(line => line.rendered).join(EOL) + EOL + EOL;
	if(internalImports.length > 0) output += internalImports.map(line => line.rendered).join(EOL) + EOL + EOL;
	
	output += EOL + lines.slice(parsed).join(EOL);
	
	fs.writeFileSync(file, output, "utf-8");
}

function parseImport(path, output, prop) {
	path = path.trim();
	
	if(path.startsWith("{")) {
		if(!path.endsWith("}")) throw new Error(`Path \`${path}\` starts with { but doesn't end with }!`);
		
		const elements = path.slice(1, -1)
												 .split(",");
		
		for(let element of elements) {
			parseImport(element, output, prop);
		}
	} else if(path.includes("::")) {
		const part = path.slice(0, path.indexOf("::"));
		output[part] ||= {};
		parseImport(path.slice(part.length + 2), output[part], prop);
	} else if(path === "self") {
		output._self = prop;
	} else {
		output[path] ||= {};
		output[path]._self = prop;
	}
}

function generateImports(imports, path, modules) {
	let superCount = path.lastIndexOf("super") + 1;
	let self = path.length > 0 && path[0] === "self";
	let crate = path.length > 0 && path[0] === "crate";
	let std = path.length > 0 && path[0] === "std";
	let internal = path.length > 0 && modules.includes(path[0]) || self || crate || superCount > 0;
	let metas = {};
	
	let output = [];
	
	for(const [name, item] of Object.entries(imports)) {
		if(name === "_self") continue;
		
		output.push(...generateImports(item, [...path, name], modules));
		
		if(item._self) {
			metas[item._self.meta] ||= { pub: [], prv: [] };
			if(item._self.pub) metas[item._self.meta].pub.push(name);
			else metas[item._self.meta].prv.push(name);
		}
	}
	
	for(const [meta, { pub, prv }] of Object.entries(metas)) {
		if(pub.length > 0) output.push(renderLine({ path, names: pub, pub: true, meta, super: superCount, self, internal, crate, std }));
		if(prv.length > 0) output.push(renderLine({ path, names: prv, pub: false, meta, super: superCount, self, internal, crate, std }));
	}
	
	return output;
}

function renderLine(line) {
	let rendered = "";
	if(line.meta) rendered += line.meta + " ";
	if(line.pub) rendered += "pub ";
	
	rendered += "use " + line.path.join("::");
	
	if(line.names.length > 1) rendered = `${rendered}::{${line.names.join(", ")}};`;
	else rendered = `${rendered}::${line.names[0]};`;
	
	return {
		...line,
		rendered,
	}
}

function generateModules(modules) {
	return Object.entries(modules).map(([name, prop]) => ({
		name,
		pub: prop.pub,
		meta: prop.meta,
		macroUse: prop.macroUse,
		rendered: `${prop.meta ? prop.meta + " " : ""}${prop.pub ? "pub " : ""}mod ${name};`,
	}))
}

function normalizeImports(imports, modules, selfPath) {
	let depth = 1;
	let pointer = imports.crate;
	
	for(let part of selfPath) {
		if(!pointer || !pointer[part]) break;
		
		let superPointer = imports;
		for(let superDepth = 0; superDepth < selfPath.length - depth; superDepth++) superPointer = (superPointer.super ||= {});
		
		mergeImports(superPointer, pointer[part]);
		delete pointer[part];
		
		pointer = superPointer;
		depth++;
	}
}

function mergeImports(dest, src) {
	for(let key of Object.keys(src)) {
		if(dest[key] && key === "_self") throw new Error("Double self import " + dest + " and " + src);
		else if(dest[key]) mergeImports(dest[key], src[key]);
		else dest[key] = src[key];
	}
}

function compareLines(a, b) {
	if(a.std !== b.std) return b.std - a.std;
	if(a.internal !== b.internal) return b.internal - a.internal;
	if(a.crate !== b.crate) return b.crate - a.crate;
	if(a.super !== b.super) return b.super - a.super;
	if(a.self !== b.self) return b.self - a.self;
	if(a.pub !== b.pub) return b.pub - a.pub;
	
	for(let i = 0; i < Math.max(a.path.length, b.path.length); i++) {
		if(!a.path[i]) return -1;
		if(!b.path[i]) return 1;
		if(a.path[i] !== b.path[i]) return a.path[i].localeCompare(b.path[i]);
	}
	
	return 0;
}

function compareModules(a, b) {
	if(a.macroUse !== b.macroUse) return b.macroUse - a.macroUse;
	if(a.pub !== b.pub) return b.pub - a.pub;
	if(a.name !== b.name) return a.name.localeCompare(b.name);
	
	return 0;
}

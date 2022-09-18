const path = require("path");
const fs = require("fs");
const cp = require("child_process");

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
const errVars = [];

for(const file of files) {
    const content = fs.readFileSync(file, "utf-8");
    const regex = /#\[derive\([\w ,]*Error[\w ,]*\)].*? enum ([a-zA-Z0-9]*) \{.*?^}/gsm;
    const lines = content.split("\n");
    let match;

    while((match = regex.exec(content))) {
        const enumName = match[1];
        const vregex = /#\[error\(.*\)].*\s(\w*)\(#\[error\(source\)]\s(?:.*::)*([\w<>]*)\)/g;

        let vmatch;
        while((vmatch = vregex.exec(match[0]))) {
            const line = content.slice(0, match.index + vmatch.index + 1).split("\n").length - 1;
            const commented = !!lines[line].match(/\s*\/\//);

            errVars.push({
                file,
                enumName,
                name: vmatch[1],
                source: vmatch[2],
                line,
                commented,
                needed: false,
            });

            if(!commented) lines[line] = "// " + lines[line];
        }
    }

    fs.writeFileSync(file, lines.join("\n"));
}

try {
    cp.execSync('cargo build', { encoding: "utf-8", stdio: [] });

    console.log("Nothing is missing!");
} catch (e) {
    const content = e.stderr;

    const regex = /the trait `From<(?:.*::)*([\w<>]*)>` is not implemented for `(?:.*::)*(\w*)`/g;
    let match;
    while((match = regex.exec(content))) {
        const errVar = errVars.filter(errVar => errVar.source === match[1] && errVar.enumName === match[2]);
        if(!errVar.length) {
            console.warn(`Couldn't find variant for ${match[2]} from ${match[1]}`);
            console.log(match[0]);
            console.log();
            continue;
        }

        errVar.forEach(ev => ev.needed = true)
    }
}

for(const errVar of errVars) {
    if(!errVar.needed) console.log(`${errVar.enumName}::${errVar.name} at ${errVar.file}@${errVar.line} is not needed!`)
}

for(const file of files) {
    const content = fs.readFileSync(file, "utf-8");
    const lines = content.split("\n");

    for(const errVar of errVars.filter(errVar => errVar.file === file)) {
        if(!errVar.needed || !lines[errVar.line].startsWith("// ")) continue;

        lines[errVar.line] = lines[errVar.line].slice(3);
    }

    fs.writeFileSync(file, lines.join("\n"));
}


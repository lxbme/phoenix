'use strict';

const vscode = require('vscode');
const path = require('path');
const { execFile } = require('child_process');
const { parse } = require('./diagnostics');

const SEVERITY = {
	error: vscode.DiagnosticSeverity.Error,
	warning: vscode.DiagnosticSeverity.Warning,
	note: vscode.DiagnosticSeverity.Information,
};

/** exit code 64 is EX_USAGE -- we passed something the binary did not understand */
const EX_USAGE = 64;

let collection;
let output;
/** remembered once a candidate answers, so the fallbacks are tried only once */
let binary = null;
let complained = false;

function activate(context) {
	collection = vscode.languages.createDiagnosticCollection('phoenix');
	output = vscode.window.createOutputChannel('Phoenix');
	context.subscriptions.push(collection, output);

	const check = (doc) => {
		if (doc.languageId === 'phoenix') {
			run(doc);
		}
	};

	context.subscriptions.push(
		vscode.workspace.onDidOpenTextDocument(check),
		vscode.workspace.onDidSaveTextDocument(check),
		vscode.workspace.onDidCloseTextDocument((doc) => collection.delete(doc.uri)),
		vscode.commands.registerCommand('phoenix.check', () => {
			const editor = vscode.window.activeTextEditor;
			if (editor) {
				run(editor.document);
			}
		}),
	);

	vscode.workspace.textDocuments.forEach(check);
}

function deactivate() {}

/**
 * Check one document and replace its squiggles.
 *
 * Runs on open and on save rather than on every keystroke, because checking
 * unsaved text would mean feeding it through `phoenix -`, and the compiler
 * rightly warns that a program read from stdin leaves no stdin for the program
 * to read -- which every program using `read` would then show as a false
 * warning. Saving is cheap enough that the difference is hard to notice.
 */
async function run(doc) {
	const config = vscode.workspace.getConfiguration('phoenix', doc.uri);
	if (!config.get('check.enable', true)) {
		collection.delete(doc.uri);
		return;
	}

	const file = doc.uri.fsPath;
	const folder = vscode.workspace.getWorkspaceFolder(doc.uri);
	const cwd = folder ? folder.uri.fsPath : path.dirname(file);

	const result = await spawn(config, cwd, file);
	if (!result) {
		return; // nothing to run; `spawn` has already said so
	}

	// The compiler reports one file and has no include mechanism, so every
	// diagnostic in the output belongs to this document.
	const items = parse(result.stderr);
	collection.set(
		doc.uri,
		items.map((item) => toDiagnostic(item, doc.uri)),
	);
}

function toRange(range) {
	return new vscode.Range(
		range.startLine,
		range.startCharacter,
		range.endLine,
		range.endCharacter,
	);
}

function toDiagnostic(item, uri) {
	const diag = new vscode.Diagnostic(
		toRange(item.range),
		item.message,
		SEVERITY[item.severity] ?? vscode.DiagnosticSeverity.Information,
	);
	diag.source = 'phoenix';
	if (item.related.length > 0) {
		// call trace frames, rendered as clickable links under the error
		diag.relatedInformation = item.related.map(
			(frame) =>
				new vscode.DiagnosticRelatedInformation(
					new vscode.Location(uri, toRange(frame.range)),
					frame.message,
				),
		);
	}
	return diag;
}

/**
 * Candidates for the binary. An explicit setting is used alone: falling back
 * from a path someone deliberately configured would hide their typo.
 */
function candidates(config, cwd) {
	const configured = String(config.get('path', '')).trim();
	if (configured !== '') {
		return [configured];
	}
	return [
		'phoenix',
		path.join(cwd, 'target', 'release', 'phoenix'),
		path.join(cwd, 'target', 'debug', 'phoenix'),
	];
}

/**
 * `-c` means analyse only. The extension never runs the program it is looking
 * at -- opening a file must not have side effects.
 */
function exec(bin, cwd, file) {
	return new Promise((resolve) => {
		execFile(
			bin,
			['-c', '--message-format=json', file],
			{ cwd, timeout: 15000 },
			(err, _stdout, stderr) => resolve({ err, stderr: stderr ?? '' }),
		);
	});
}

async function spawn(config, cwd, file) {
	const tries = binary ? [binary] : candidates(config, cwd);
	for (const bin of tries) {
		const result = await exec(bin, cwd, file);
		if (result.err && result.err.code === 'ENOENT') {
			continue; // not there; try the next candidate
		}
		if (result.err && result.err.code === EX_USAGE) {
			// a non-zero exit is normal here -- it is how "did not compile" is
			// reported -- but EX_USAGE means the arguments were rejected
			say(
				`\`${bin}\` did not accept \`--message-format\`. Rebuild it, or point \`phoenix.path\` at a newer binary.`,
			);
			output.appendLine(result.stderr);
			return null;
		}
		binary = bin;
		return result;
	}
	say('Could not find the `phoenix` binary. Set `phoenix.path` to point at it.');
	output.appendLine(`tried: ${tries.join(', ')}`);
	return null;
}

/** Said once per session; a warning on every save would be worse than silence. */
function say(message) {
	output.appendLine(message);
	if (!complained) {
		complained = true;
		vscode.window.showWarningMessage(`Phoenix: ${message}`);
	}
}

module.exports = { activate, deactivate };

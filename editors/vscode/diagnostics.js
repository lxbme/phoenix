'use strict';

// Turning `phoenix --message-format=json` output into something the editor can
// draw. Kept free of `require('vscode')` so it can be exercised under plain
// node; `extension.js` is the only part that needs the editor to exist.

/**
 * Parse the compiler's line-delimited JSON into plain objects.
 *
 * A `note` is not a finding of its own -- it is a frame of the call trace
 * belonging to the diagnostic printed above it -- so it is folded into that
 * diagnostic's `related` list rather than becoming a separate squiggle. A note
 * with nothing above it is kept on its own, which should not happen but is not
 * worth losing.
 *
 * Positions come out 0-based, which is what the editor wants; the compiler
 * emits them 1-based to match its own human output.
 *
 * @param {string} text the compiler's stderr
 * @returns {Array<object>}
 */
function parse(text) {
	const out = [];
	for (const line of String(text).split('\n')) {
		if (line.trim() === '') {
			continue;
		}
		let raw;
		try {
			raw = JSON.parse(line);
		} catch {
			continue; // not ours: a panic, a warning from the shell, anything
		}
		if (!raw || typeof raw.line_start !== 'number') {
			continue;
		}

		const item = {
			severity: raw.severity,
			// the editor has no second field for the compiler's `= note:` line,
			// so it rides along under the message and shows up in the hover
			message: raw.note ? `${raw.message}\n${raw.note}` : raw.message,
			file: raw.file,
			range: {
				startLine: raw.line_start - 1,
				startCharacter: raw.column_start - 1,
				endLine: raw.line_end - 1,
				endCharacter: raw.column_end - 1,
			},
			related: [],
		};

		if (item.severity === 'note' && out.length > 0) {
			out[out.length - 1].related.push(item);
		} else {
			out.push(item);
		}
	}
	return out;
}

module.exports = { parse };

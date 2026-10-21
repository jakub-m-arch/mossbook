// Keeps the open entry form across a lock (decision D-20). The form is kept,
// encrypted, while it is being filled in; leaving it on purpose discards it.
// Calls run one after another, so a late "keep" never lands after a "discard".

import { unsavedDiscard, unsavedKeep } from './api/index.js';
import type { EntryDraft } from './api/index.js';

let queue: Promise<unknown> = Promise.resolve();
let flushOpenForm: (() => void) | null = null;

function enqueue(call: () => Promise<unknown>): Promise<void> {
	// While locked the call fails; the copy kept before the lock stays.
	queue = queue.then(call).catch(() => {});
	return queue.then(() => {});
}

/** Keeps `draft` for the next unlock, replacing the previous copy. */
export function keepUnsaved(draft: EntryDraft): Promise<void> {
	return enqueue(() => unsavedKeep(draft));
}

/** Forgets the kept copy (the form was left on purpose). */
export function discardUnsaved(): Promise<void> {
	return enqueue(() => unsavedDiscard());
}

/** The open form registers how to keep its latest state at once. */
export function setFlush(flush: (() => void) | null): void {
	flushOpenForm = flush;
}

/** Keeps the open form now, e.g. just before a manual lock. */
export function flushUnsaved(): Promise<void> {
	flushOpenForm?.();
	return queue.then(() => {});
}

/**
 * Called before leaving an entry form: going to another screen discards the
 * kept copy; a lock (the lock screen) or closing the app keeps it.
 */
export function leaveForm(to: { route: { id: string | null } } | null): void {
	if (to?.route.id && to.route.id !== '/lock') void discardUnsaved();
}

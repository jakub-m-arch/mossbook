// One message line per section of a settings screen: the result of an action
// or its translated error. A locked journal goes to the lock screen instead.
import { errorText, handleLocked } from '#lib/session.svelte.js';

export function createMessages() {
	const messages = $state<Record<string, string>>({});

	/** Runs `action`; its returned text (or the error) becomes the section's message. */
	async function attempt(section: string, action: () => Promise<string | void>) {
		messages[section] = '';
		try {
			messages[section] = (await action()) ?? '';
		} catch (e) {
			if (!(await handleLocked(e))) messages[section] = errorText(e);
		}
	}

	return { messages, attempt };
}

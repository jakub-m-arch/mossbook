// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			/** Continue the entry form kept when the journal locked (D-20). */
			resume?: boolean;
		}
		// interface Platform {}
	}
}

export {};

// Grounding exercises (P-4): breathing patterns and the 5-4-3-2-1 steps.

export type Phase = { kind: 'inhale' | 'hold' | 'exhale'; seconds: number };

export const BREATHING: Record<'478' | 'box', { title: string; intro: string; phases: Phase[] }> = {
	'478': {
		title: 'exercise.breathe478',
		intro: 'exercise.intro478',
		phases: [
			{ kind: 'inhale', seconds: 4 },
			{ kind: 'hold', seconds: 7 },
			{ kind: 'exhale', seconds: 8 }
		]
	},
	box: {
		title: 'exercise.breatheBox',
		intro: 'exercise.introBox',
		phases: [
			{ kind: 'inhale', seconds: 4 },
			{ kind: 'hold', seconds: 4 },
			{ kind: 'exhale', seconds: 4 },
			{ kind: 'hold', seconds: 4 }
		]
	}
};

export const GROUNDING_STEPS = ['see', 'touch', 'hear', 'smell', 'taste'] as const;

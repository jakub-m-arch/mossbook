// Builds the therapist report (X-3) from entries. The same model is shown as the
// preview and rendered to PDF by Rust, so what the user sees is what is saved.

import type {
	Emotion,
	Entry,
	ReportDoc,
	ReportField,
	ReportItem,
	Symptom,
	Tag
} from './api/index.js';
import { groupByDay, kindTag } from './entries.js';

type Translate = (key: string, params?: Record<string, string | number>) => string;

export interface ReportOptions {
	/** Local `YYYY-MM-DD`, inclusive. */
	from: string;
	to: string;
	tags: Map<string, Tag>;
	t: Translate;
	locale: string;
	/** Whether the PDF file will be password-protected (changes the footer). */
	encrypted?: boolean;
}

const chip = (t: Translate, prefix: string, item: Emotion | Symptom) =>
	typeof item === 'string' ? t(`${prefix}.${item}`) : item.custom;

/** Report items read top to bottom in time, so entries go oldest first. */
export function buildReport(entries: Entry[], o: ReportOptions): ReportDoc {
	const { t } = o;
	const time = new Intl.DateTimeFormat(o.locale, { hour: '2-digit', minute: '2-digit' });
	const day = new Intl.DateTimeFormat(o.locale, {
		weekday: 'long',
		day: 'numeric',
		month: 'long',
		year: 'numeric'
	});
	const date = new Intl.DateTimeFormat(o.locale, {
		day: 'numeric',
		month: 'long',
		year: 'numeric'
	});
	const number = new Intl.NumberFormat(o.locale, { maximumFractionDigits: 1 });
	const localDate = (d: string) => new Date(`${d}T12:00`);

	const sorted = [...entries].sort((a, b) => Date.parse(a.occurred_at) - Date.parse(b.occurred_at));
	// groupByDay keeps the input order, so groups come out oldest first too.
	const groups = groupByDay(sorted);

	const moods = sorted.map((e) => e.mood).filter((m): m is number => m !== null);
	const summary = [t('report.count', { count: sorted.length })];
	if (moods.length > 0) {
		const average = moods.reduce((a, b) => a + b, 0) / moods.length;
		summary.push(t('report.moodAverage', { average: number.format(average) }));
	}

	return {
		title: t('report.docTitle'),
		subtitle: t('report.period', {
			from: date.format(localDate(o.from)),
			to: date.format(localDate(o.to))
		}),
		summary,
		sections: groups.map((g) => ({
			heading: day.format(localDate(g.key)),
			items: g.items.map((e) => item(e, o, time))
		})),
		footer: t(o.encrypted ? 'report.footerEncrypted' : 'report.footer'),
		page_label: t('report.page')
	};
}

function item(e: Entry, o: ReportOptions, time: Intl.DateTimeFormat): ReportItem {
	const { t } = o;
	const kind = kindTag(e.kind);
	const meta: string[] = [];
	const fields: ReportField[] = [];
	const field = (label: string, value: string | null | undefined) => {
		if (value && value.trim()) fields.push({ label, value: value.trim() });
	};

	if (e.mood !== null) meta.push(t('report.mood', { label: t(`mood.${e.mood}`) }));
	if (e.intensity !== null) meta.push(t('report.intensity', { value: e.intensity }));

	const k = e.kind;
	if (typeof k !== 'string') {
		if ('mood' in k) {
			if (k.mood.energy !== null) meta.push(t('report.energy', { value: k.mood.energy }));
			field(t('emotion.label'), k.mood.emotions.map((x) => chip(t, 'emotion', x)).join(', '));
		} else if ('event' in k) {
			for (const key of ['what', 'place', 'body', 'thoughts', 'helped'] as const) {
				field(t(`field.${key}`), k.event[key]);
			}
		} else if ('fear' in k) {
			field(t('field.trigger'), k.fear.trigger);
			field(t('symptom.label'), k.fear.symptoms.map((x) => chip(t, 'symptom', x)).join(', '));
			if (k.fear.avoidance !== null) {
				meta.push(k.fear.avoidance ? t('report.avoidanceYes') : t('report.avoidanceNo'));
			}
			field(t('field.strategy'), k.fear.strategy);
			if (k.fear.intensity_after !== null) {
				meta.push(t('report.after', { value: k.fear.intensity_after }));
			}
		} else if ('sleep' in k) {
			const s = k.sleep;
			if (s.bedtime && s.wake) {
				meta.push(
					t('report.sleepTime', {
						from: time.format(new Date(s.bedtime)),
						to: time.format(new Date(s.wake))
					})
				);
			}
			if (s.quality !== null) meta.push(t('report.quality', { value: s.quality }));
			if (s.awakenings !== null) meta.push(t('report.awakenings', { value: s.awakenings }));
			if (s.nightmare) meta.push(t('report.nightmareYes'));
		} else if ('nightmare' in k) {
			field(t('field.content'), k.nightmare.content);
			field(t('field.rescripted'), k.nightmare.rescripted);
		}
	}
	field(t('field.note'), e.note);

	const tagNames = e.tags.map((id) => o.tags.get(id)?.name).filter(Boolean);
	if (tagNames.length > 0) meta.push(t('report.tagsLine', { tags: tagNames.join(', ') }));
	if (e.incomplete) meta.push(t('report.incomplete'));

	return {
		title: `${time.format(new Date(e.occurred_at))} · ${t(`kind.${kind}`)}`,
		meta: meta.join(' · '),
		fields
	};
}

import { describe, expect, it } from 'vitest';
import { APP_NAME } from './index';

describe('APP_NAME', () => {
	it('is Mossbook', () => {
		expect(APP_NAME).toBe('Mossbook');
	});
});

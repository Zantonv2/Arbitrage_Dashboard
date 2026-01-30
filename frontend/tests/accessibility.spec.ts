import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test.describe('Accessibility', () => {
	test('dashboard meets WCAG 2.1 AAA', async ({ page }) => {
		await page.goto('/');
		const results = await new AxeBuilder({ page }).withTags(['wcag2aaa']).analyze();
		expect(results.violations).toEqual([]);
	});

	test('signals page meets WCAG 2.1 AAA', async ({ page }) => {
		await page.goto('/signals');
		const results = await new AxeBuilder({ page }).withTags(['wcag2aaa']).analyze();
		expect(results.violations).toEqual([]);
	});

	test('exchanges page meets WCAG 2.1 AAA', async ({ page }) => {
		await page.goto('/exchanges');
		const results = await new AxeBuilder({ page }).withTags(['wcag2aaa']).analyze();
		expect(results.violations).toEqual([]);
	});

	test('strategies page meets WCAG 2.1 AAA', async ({ page }) => {
		await page.goto('/strategies');
		const results = await new AxeBuilder({ page }).withTags(['wcag2aaa']).analyze();
		expect(results.violations).toEqual([]);
	});

	test('keyboard navigation works', async ({ page }) => {
		await page.goto('/');
		
		// Tab through interactive elements
		await page.keyboard.press('Tab');
		await expect(page.locator(':focus')).toBeVisible();
		
		// Check focus indicators are visible
		const focusedElement = page.locator(':focus');
		const outline = await focusedElement.evaluate((el) => 
			window.getComputedStyle(el).outline
		);
		expect(outline).not.toBe('none');
	});
});

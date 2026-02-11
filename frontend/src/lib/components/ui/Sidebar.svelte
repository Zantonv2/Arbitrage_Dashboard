<script lang="ts">
	import { page } from '$app/state';
	import { t, getLanguage } from '$lib/i18n.svelte';
	import type { Language } from '$lib/i18n.svelte';

	interface NavItem {
		label: string;
		href: string;
		icon: string;
	}

	let currentLanguage = $state<Language>('en');

	// Reactive nav items that update with language
	const navItems = $derived([
		{ label: t('nav.dashboard'), href: '/', icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6' },
		{ label: t('nav.signals'), href: '/signals', icon: 'M13 10V3L4 14h7v7l9-11h-7z' },
		{ label: t('nav.exchanges'), href: '/exchanges', icon: 'M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01' },
		{ label: t('nav.settings'), href: '/settings', icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z' }
	]);

	let isExpanded = $state(true);

	// Update language when it changes
	$effect(() => {
		currentLanguage = getLanguage();
	});

	function toggleSidebar(e: MouseEvent): void {
		e.preventDefault();
		e.stopPropagation();
		isExpanded = !isExpanded;
	}
</script>

<aside 
	class="sidebar glass"
	class:collapsed={!isExpanded}
	style="width: {isExpanded ? 'var(--sidebar-width)' : 'var(--sidebar-collapsed-width)'}"
	data-sidebar-expanded={isExpanded}
	role="navigation"
>
	<!-- Logo Section - Only collapse trigger -->
	<div class="sidebar-header">
		<button 
			class="logo-container" 
			onclick={toggleSidebar}
			title={isExpanded ? t('sidebar.collapse') : t('sidebar.expand')}
			type="button"
		>
			<div class="logo-icon">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			{#if isExpanded}
				<span class="logo-text">Triangulum</span>
			{/if}
		</button>
	</div>
	
	<!-- Navigation -->
	<nav class="sidebar-nav">
		{#each navItems as item}
			<a 
				href={item.href} 
				class="nav-item"
				class:active={page.url.pathname === item.href}
				title={!isExpanded ? item.label : ''}
			>
				<div class="nav-icon-container" class:active={page.url.pathname === item.href}>
					<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d={item.icon} stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</div>
				{#if isExpanded}
					<span class="nav-label">{item.label}</span>
				{/if}
			</a>
		{/each}
	</nav>
</aside>

<style>
	.sidebar {
		height: 100vh;
		position: fixed;
		left: 0;
		top: 0;
		display: flex;
		flex-direction: column;
		transition: width var(--duration-normal) var(--ease-emphasized);
		z-index: 50;
		border-right: 1px solid var(--glass-border);
	}
	
	.sidebar-header {
		padding: var(--space-3);
		border-bottom: 1px solid var(--glass-border);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	
	.logo-container {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		overflow: hidden;
		background: transparent;
		border: none;
		cursor: pointer;
		padding: var(--space-2);
		margin: calc(-1 * var(--space-2));
		border-radius: var(--shape-large);
		transition: all var(--duration-fast) var(--ease-standard);
		text-decoration: none;
		flex: 1;
		justify-content: flex-start;
		color: inherit;
	}
	
	.logo-container:hover {
		background-color: var(--md-sys-color-surface-container-high);
	}
	
	.logo-icon {
		width: 32px;
		height: 32px;
		border-radius: var(--shape-medium);
		background: linear-gradient(135deg, var(--md-sys-color-primary), var(--md-sys-color-tertiary));
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--md-sys-color-on-primary);
		flex-shrink: 0;
	}
	
	.logo-icon svg {
		width: 18px;
		height: 18px;
	}
	
	.logo-text {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
		font-weight: 600;
		white-space: nowrap;
	}
	
	.sidebar-nav {
		flex: 1;
		padding: var(--space-2);
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		overflow-y: auto;
	}
	
	.nav-item {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2);
		border-radius: var(--shape-large);
		color: var(--md-sys-color-on-surface-variant);
		text-decoration: none;
		transition: all var(--duration-fast) var(--ease-standard);
	}
	
	.nav-item:hover {
		background-color: var(--md-sys-color-surface-container-high);
		color: var(--md-sys-color-on-surface);
	}
	
	.nav-item.active {
		background-color: transparent;
		color: var(--md-sys-color-on-primary-container);
	}
	
	.nav-icon-container {
		width: 40px;
		height: 40px;
		border-radius: var(--shape-medium);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		transition: all var(--duration-fast) var(--ease-standard);
	}
	
	.nav-item:hover .nav-icon-container {
		background-color: var(--md-sys-color-surface-container-highest);
	}
	
	.nav-item.active .nav-icon-container {
		background: linear-gradient(135deg, var(--md-sys-color-primary), var(--md-sys-color-tertiary));
		color: var(--md-sys-color-on-primary);
		box-shadow: 0 2px 8px rgba(103, 80, 164, 0.3);
	}
	
	.nav-icon {
		width: 20px;
		height: 20px;
		transition: color var(--duration-fast) var(--ease-standard);
	}
	
	.nav-label {
		font: var(--typography-body-medium);
		white-space: nowrap;
	}
	
	/* Collapsed state - icons centered, no size change */
	.sidebar.collapsed .nav-item {
		justify-content: center;
		padding: var(--space-2) 0;
	}
	
	.sidebar.collapsed .nav-icon-container {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	
	.sidebar.collapsed .nav-icon {
		width: 20px;
		height: 20px;
	}
	
	.sidebar.collapsed .nav-label {
		display: none;
	}
	
	.sidebar.collapsed .sidebar-header {
		padding: var(--space-3);
	}
	
	.sidebar.collapsed .logo-container {
		justify-content: center;
		padding: var(--space-1);
	}
	
	.sidebar.collapsed .logo-icon {
		width: 32px;
		height: 32px;
	}
	
	.sidebar.collapsed .logo-icon svg {
		width: 18px;
		height: 18px;
	}
</style>

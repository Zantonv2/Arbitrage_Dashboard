<script lang="ts">
	import { page } from '$app/state';

	interface NavItem {
		label: string;
		href: string;
		icon: string;
	}

	const navItems: NavItem[] = [
		{ label: 'Dashboard', href: '/', icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6' },
		{ label: 'Signals', href: '/signals', icon: 'M13 10V3L4 14h7v7l9-11h-7z' },
		{ label: 'Exchanges', href: '/exchanges', icon: 'M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01' },
		{ label: 'Strategies', href: '/strategies', icon: 'M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z' },
		{ label: 'Analytics', href: '/analytics', icon: 'M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z' },
		{ label: 'Settings', href: '/settings', icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z' }
	];

	let isExpanded = $state(true);
	
	function toggleSidebar() {
		isExpanded = !isExpanded;
	}
</script>

<aside 
	class="sidebar glass glass-elevation-2"
	class:collapsed={!isExpanded}
	style="width: {isExpanded ? 'var(--sidebar-width)' : 'var(--sidebar-collapsed-width)'}"
>
	<!-- Logo Section -->
	<div class="sidebar-header">
		<div class="logo-container">
			<div class="logo-icon">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			{#if isExpanded}
				<span class="logo-text">Arbitrage</span>
			{/if}
		</div>
		<button 
			class="toggle-btn"
			onclick={toggleSidebar}
			aria-label={isExpanded ? 'Collapse sidebar' : 'Expand sidebar'}
		>
			<svg 
				viewBox="0 0 24 24" 
				fill="none" 
				stroke="currentColor" 
				stroke-width="2"
				class="toggle-icon"
				class:rotated={!isExpanded}
			>
				<path d="M11 17l-5-5 5-5M18 17l-5-5 5-5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
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
				<div class="nav-icon">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d={item.icon} stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</div>
				{#if isExpanded}
					<span class="nav-label">{item.label}</span>
				{/if}
			</a>
		{/each}
	</nav>
	
	<!-- Footer -->
	<div class="sidebar-footer">
		{#if isExpanded}
			<div class="version-info">
				<span class="version-label">v0.1.0</span>
				<span class="status-badge">
					<span class="status-dot"></span>
					Connected
				</span>
			</div>
		{:else}
			<div class="status-dot-mini"></div>
		{/if}
	</div>
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
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-4);
		border-bottom: 1px solid var(--glass-border);
	}
	
	.logo-container {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		overflow: hidden;
	}
	
	.logo-icon {
		width: 36px;
		height: 36px;
		border-radius: var(--shape-medium);
		background: linear-gradient(135deg, var(--md-sys-color-primary), var(--md-sys-color-tertiary));
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--md-sys-color-on-primary);
		flex-shrink: 0;
	}
	
	.logo-icon svg {
		width: 20px;
		height: 20px;
	}
	
	.logo-text {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		font-weight: 600;
		white-space: nowrap;
	}
	
	.toggle-btn {
		width: 32px;
		height: 32px;
		border-radius: var(--shape-full);
		border: none;
		background: transparent;
		color: var(--md-sys-color-on-surface-variant);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all var(--duration-fast) var(--ease-standard);
		flex-shrink: 0;
	}
	
	.toggle-btn:hover {
		background-color: var(--md-sys-color-surface-container-high);
		color: var(--md-sys-color-on-surface);
	}
	
	.toggle-icon {
		width: 18px;
		height: 18px;
		transition: transform var(--duration-normal) var(--ease-emphasized);
	}
	
	.toggle-icon.rotated {
		transform: rotate(180deg);
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
		padding: var(--space-3) var(--space-3);
		border-radius: var(--shape-large);
		color: var(--md-sys-color-on-surface-variant);
		text-decoration: none;
		transition: all var(--duration-fast) var(--ease-standard);
		position: relative;
		overflow: hidden;
	}
	
	.nav-item:hover {
		background-color: var(--md-sys-color-surface-container-high);
		color: var(--md-sys-color-on-surface);
	}
	
	.nav-item.active {
		background-color: var(--md-sys-color-secondary-container);
		color: var(--md-sys-color-on-secondary-container);
	}
	
	.nav-item.active::before {
		content: '';
		position: absolute;
		left: 0;
		top: 50%;
		transform: translateY(-50%);
		width: 3px;
		height: 20px;
		background-color: var(--md-sys-color-primary);
		border-radius: 0 var(--shape-full) var(--shape-full) 0;
	}
	
	.nav-icon {
		width: 24px;
		height: 24px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	
	.nav-icon svg {
		width: 20px;
		height: 20px;
	}
	
	.nav-label {
		font: var(--typography-label-large);
		white-space: nowrap;
	}
	
	.sidebar-footer {
		padding: var(--space-4);
		border-top: 1px solid var(--glass-border);
	}
	
	.version-info {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	
	.version-label {
		font: var(--typography-label-sm);
		color: var(--md-sys-color-on-surface-variant);
	}
	
	.status-badge {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		font: var(--typography-label-sm);
		color: var(--color-profit);
	}
	
	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: var(--shape-full);
		background-color: var(--color-profit);
		animation: pulse-live 2s ease-in-out infinite;
	}
	
	.status-dot-mini {
		width: 8px;
		height: 8px;
		border-radius: var(--shape-full);
		background-color: var(--color-profit);
		animation: pulse-live 2s ease-in-out infinite;
		margin: 0 auto;
	}
	
	/* Collapsed state adjustments */
	.sidebar.collapsed .nav-item {
		justify-content: center;
		padding: var(--space-3);
	}
	
	.sidebar.collapsed .nav-item.active::before {
		display: none;
	}
	
	@keyframes pulse-live {
		0%, 100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}
</style>

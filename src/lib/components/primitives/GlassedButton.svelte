<script lang="ts">
	import { fade } from 'svelte/transition';

	let { contrast = 'dark', styles }: { contrast?: string; styles?: any } = $props();

	let isHovering = $state(false);
	let isDark = $derived(contrast == 'dark' || contrast == 'dark-contrast');

	let textColor = $state('text-white');
	$effect(() => {
		switch (contrast) {
			case 'light-contrast':
				textColor = 'text-black';
				break;
			case 'dark-contrast':
				textColor = 'text-black/50';
				break;
			default:
				textColor = 'text-white';
				break;
		}
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	onmouseenter={() => isHovering = true}
	onmouseleave={() => isHovering = false}
	class="button-wrap relative overscroll-contain"
>
	{#if isHovering}
		<div transition:fade class="hoverstyle absolute inset-0 bg-white/[0.05] rounded-[22px] pointer-events-none z-10">
			<div
				class="rotating-gradient absolute inset-0 rounded-[22px]"
				style="
					mix-blend-mode: overlay;
					opacity: 0.15;
					background: conic-gradient(
						from 0deg,
						#e7ffff 0%,
						{styles?.['color']?.accent || '#aaf'} 25%,
						#fff 50%,
						{styles?.['color']?.accent || '#aaf'} 75%,
						#e7ffff 100%
					);
					animation: rotate-gradient 4s linear infinite;
				"
			></div>
		</div>
	{/if}

	<button
		class="glassy-button overflow-hidden {isDark ? 'dark-glassy-button' : 'light-glassy-button'}"
		style="border-radius: {styles?.style?.roundness || 22}px;"
	>
		<span
			class="{textColor} text-nowrap"
			style="
				font-family: {styles?.text?.font_family || 'system-ui'};
				letter-spacing: -0.02em;
				font-weight: {styles?.text?.size_weight || 500};
				font-size: {styles?.text?.font_size || 0.9}rem;
				padding-inline: {styles?.style?.padding_x || 1.5}rem;
				padding-block: {styles?.style?.padding_y || 0.8}rem;
			"
		>
			<div class="relative z-30">
				{styles?.text?.content || ''}
			</div>
		</span>
	</button>
	<div class="glass-backdrop" style="border-radius: {styles?.style?.roundness || 22}px;"></div>
</div>

<style>
	@keyframes rotate-gradient {
		0% { transform: rotate(0deg); }
		100% { transform: rotate(360deg); }
	}

	.button-wrap {
		position: relative;
		display: inline-block;
		overflow: visible;
		background: transparent;
		transition: all 300ms cubic-bezier(0.25, 1, 0.5, 1);
		width: fit-content;
	}

	.button-wrap:hover {
		transform: translateY(-1px);
	}

	.button-wrap:active {
		transform: translateY(0) scale(0.98);
	}

	.glass-backdrop {
		position: absolute;
		inset: 0;
		z-index: 0;
		-webkit-backdrop-filter: blur(12px);
		backdrop-filter: blur(12px);
		background: linear-gradient(
			135deg,
			rgba(255, 255, 255, 0.06) 0%,
			rgba(255, 255, 255, 0.015) 100%
		);
		border: 1px solid rgba(255, 255, 255, 0.08);
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.1),
			inset 0 -1px 0 rgba(0, 0, 0, 0.15),
			0 4px 12px rgba(0, 0, 0, 0.2);
		pointer-events: none;
	}

	.glassy-button {
		all: unset;
		cursor: pointer;
		position: relative;
		pointer-events: auto;
		z-index: 3;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.glassy-button span {
		position: relative;
		display: block;
		user-select: none;
		-webkit-user-select: none;
	}
</style>

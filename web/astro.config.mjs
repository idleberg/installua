// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'Installua',
			logo: {
				src: './public/installua.svg',
			},
			customCss: ['./src/styles/custom.css'],
			// One theme, so neither the toggle nor the provider that honours it
			// has anything to do. See `src/components/NoTheme.astro`.
			components: {
				ThemeProvider: './src/components/NoTheme.astro',
				ThemeSelect: './src/components/NoTheme.astro',
			},
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/idleberg/installua' }],
       expressiveCode: {
    themes: ['github-dark'],
  },
			sidebar: [
				{
					label: 'Getting started',
					items: [
						{ label: 'Why Installua?', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Editor Setup', slug: 'getting-started/editor-setup' },
						{ label: 'Create project', slug: 'getting-started/create-project' },
					],
				},
				{
					label: 'Advanced Usage',
					items: [
						{ label: 'Command line', slug: 'cli' },
						{ label: 'installua.toml', slug: 'project-file' },
						{ label: 'Agent skills', slug: 'skills' },
					],
				},

				{
					label: 'Concepts',
					items: [
						{ label: 'Lua-shaped, not Lua', slug: 'concepts/lua-shaped-not-lua' },
						{ label: 'NSIS-shaped, not NSIS', slug: 'concepts/nsis-shaped-not-nsis' },
					],
				},
				{
					label: 'Reference',
					items: [
						{
							label: 'Commands',
							collapsed: true,
							items: [
								{ label: 'Overview', slug: 'reference/commands' },
								{ label: 'Program structure', slug: 'reference/commands/program-structure' },
								{ label: 'Script attributes', slug: 'reference/commands/script-attributes' },
								{ label: 'Sections and install types', slug: 'reference/commands/sections-and-install-types' },
								{ label: 'Windows and controls', slug: 'reference/commands/windows-and-controls' },
								{ label: 'Languages and locales', slug: 'reference/commands/languages-and-locales' },
								{ label: 'Files and directories', slug: 'reference/commands/files-and-directories' },
								{ label: 'Files on the target at runtime', slug: 'reference/commands/files-on-the-target-at-runtime' },
								{ label: 'Registry and INI', slug: 'reference/commands/registry-and-ini' },
								{ label: 'Processes and the shell', slug: 'reference/commands/processes-and-the-shell' },
								{ label: 'Strings and numbers', slug: 'reference/commands/strings-and-numbers' },
								{ label: 'Flow, errors and messages', slug: 'reference/commands/flow-errors-and-messages' },
								{ label: 'Windows facts', slug: 'reference/commands/windows-facts' },
								{ label: 'Plugins and headers', slug: 'reference/commands/plugins-and-headers' },
								{ label: 'Constants', slug: 'reference/commands/constants' },
								{ label: 'Not available', slug: 'reference/commands/not-available' },
							],
						},
						{
              label: 'Plugins',
							collapsed: true,
							items: [
                { label: 'Overview', slug: 'reference/plugins' },
								{ label: 'Tagged outputs', slug: 'reference/plugins/tagged-outputs' },
								{ label: 'Shipped with NSIS', slug: 'reference/plugins/plugins-that-ship-with-nsis' },
								{ label: 'Third-party', slug: 'reference/plugins/third-party-plugins' },
								{ label: 'What stays out', slug: 'reference/plugins/what-stays-out' },
							],
						},
						{ label: 'Headers', slug: 'reference/headers' },
            { label: 'Modern UI', slug: 'reference/modern-ui' },
					],
				},
			],
		}),
	],
});

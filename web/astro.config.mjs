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
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/withastro/starlight' }],
       expressiveCode: {
    themes: ['github-dark'],
  },
			sidebar: [
				{
					label: 'Getting started',
					items: [
						// Each item here is one entry in the navigation menu.
						{ label: 'Why Installua?', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Editor Setup', slug: 'getting-started/editor-setup' },
					],
				},
				{
					label: 'Reference',
					items: [{ autogenerate: { directory: 'reference' } }],
				},
			],
		}),
	],
});

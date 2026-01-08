import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'Shieldnet',
  tagline: 'Enforce transaction security onchain',
  // TODO replace with favicon
  favicon: 'img/favicon.ico',

  // Future flags, see https://docusaurus.io/docs/api/docusaurus-config#future
  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  // Set the production url of your site here 
  // TODO: Update once known
  url: 'https://your-docusaurus-site.example.com',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: 'safe-research', // Usually your GitHub org/user name.
  projectName: 'shieldnet', // Usually your repo name.

  onBrokenLinks: 'throw',

  // Even if you don't use internationalization, you can use this field to set
  // useful metadata like html lang. For example, if your site is Chinese, you
  // may want to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl:
            'https://github.com/safe-research/shieldnet/tree/main/docs/public/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    // TODO: Replace with your project's social card
    // image: 'img/docusaurus-social-card.jpg', 
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Shieldnet',
      logo: {
        alt: 'Shieldnet Logo',
        // TODO replace with shieldnet logo
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          href: 'https://github.com/safe-research/shieldnet',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Community',
          items: [
            {
              label: 'Safe Ecosystem Foundation',
              href: 'https://safefoundation.org/',
            },
            {
              label: 'Imprint',
              href: 'https://safefoundation.org/imprint',
            },
            {
              label: 'X',
              href: 'https://x.com/safe',
            },
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'Safe Research',
              to: 'https://safe.dev',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/safe-research/shieldnet',
            },
          ],
        },
      ],
      copyright: `©2025 Safe Ecosystem Foundation`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;

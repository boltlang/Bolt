import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'The Bolt Programming Language',
  tagline: 'A highly experimental programming language for safe and rapid application development',
  favicon: 'img/favicon.ico',

  // Future flags, see https://docusaurus.io/docs/api/docusaurus-config#future
  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  // Set the production url of your site here
  url: 'https://boltlang.net',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: 'boltlang', // Usually your GitHub org/user name.
  projectName: 'bolt', // Usually your repo name.

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  // Even if you don't use internationalization, you can use this field to set
  // useful metadata like html lang. For example, if your site is Chinese, you
  // may want to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  // The internals were reorganized by subject: compiler stages, designs and
  // tooling. Old links keep working.
  plugins: [
    [
      '@docusaurus/plugin-client-redirects',
      {
        redirects: [
          {from: '/docs/internals/type-checking/arguments', to: '/docs/internals/designs/parameter-names'},
          {from: '/docs/internals/type-checking/mutability/prior-art', to: '/docs/internals/designs/mutability/prior-art'},
          {from: '/docs/internals/type-checking/mutability/functions', to: '/docs/internals/compiler/type-checking/mutability/functions'},
          {from: '/docs/internals/type-checking/mutability/implementation', to: '/docs/internals/compiler/type-checking/mutability/implementation'},
          {from: '/docs/internals/type-checking/mutability/errors', to: '/docs/internals/compiler/type-checking/mutability/errors'},
          {from: '/docs/internals/type-checking/mutability/uniqueness', to: '/docs/internals/compiler/conduit'},
          {from: '/docs/internals/type-checking/mutability/regions', to: '/docs/internals/compiler/conduit'},
          {from: '/docs/internals/type-checking/mutability', to: '/docs/internals/designs/mutability'},
          {from: '/docs/internals/conduit', to: '/docs/internals/compiler/conduit'},
          {from: '/docs/internals/effects', to: '/docs/internals/designs/effects'},
          {from: '/docs/internals/async', to: '/docs/internals/designs/async'},
          {from: '/docs/internals/reactive', to: '/docs/internals/designs/reactive'},
          {from: '/docs/internals/packages', to: '/docs/internals/tooling/packages'},
          {from: '/docs/internals/type-checking/constraints', to: '/docs/internals/compiler/type-checking/constraints'},
          {from: '/docs/internals/type-checking/errors', to: '/docs/internals/compiler/type-checking/errors'},
          {from: '/docs/internals/type-checking/generalization', to: '/docs/internals/compiler/type-checking/generalization'},
          {from: '/docs/internals/type-checking/inference', to: '/docs/internals/compiler/type-checking/inference'},
          {from: '/docs/internals/type-checking/kinds', to: '/docs/internals/compiler/type-checking/kinds'},
          {from: '/docs/internals/type-checking/types', to: '/docs/internals/compiler/type-checking/types'},
          {from: '/docs/internals/type-checking/unification', to: '/docs/internals/compiler/type-checking/unification'},
          {from: '/docs/internals/type-checking', to: '/docs/internals/compiler/type-checking'},
        ],
      },
    ],
  ],

  presets: [
    [
      'classic',
      {
        docs: {
          remarkPlugins: [remarkMath],
          rehypePlugins: [rehypeKatex],
          sidebarPath: './sidebars.ts',
          // Remove this to remove the "edit this page" links.
          editUrl:
            'https://github.com/boltlang/Bolt/tree/main/site/',
        },
        blog: {
          showReadingTime: true,
          feedOptions: {
            type: ['rss', 'atom'],
            xslt: true,
          },
          // Remove this to remove the "edit this page" links.
          editUrl:
            'https://github.com/boltlang/Bolt/tree/main/site/',
          // Useful options to enforce blogging best practices
          onInlineTags: 'warn',
          onInlineAuthors: 'warn',
          onUntruncatedBlogPosts: 'warn',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  stylesheets: [
    {
      href: 'https://cdn.jsdelivr.net/npm/katex@0.16.47/dist/katex.min.css',
      type: 'text/css',
      integrity:
        'sha384-nH0MfJ44wi1dd7w6jinlyBgljjS8EJAh2JBoRad8a3VDw2K69vfaaqm4WnR+gXtA',
      crossorigin: 'anonymous',
    },
  ],

  themeConfig: {
    image: 'img/bolt-banner.png',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Bolt',
      logo: {
        alt: 'The Bolt Programming Language Logo',
        src: 'img/logo.png',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'tutorialSidebar',
          position: 'left',
          label: 'Tutorial',
        },
        {
          type: 'docSidebar',
          sidebarId: 'referenceSidebar',
          position: 'left',
          label: 'Reference',
        },
        {to: '/blog', label: 'Blog', position: 'left'},
        {
          type: 'docSidebar',
          sidebarId: 'internalsSidebar',
          position: 'right',
          label: 'Internals',
        },
        {
          href: 'https://github.com/boltlang/bolt',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {
              label: 'Tutorial',
              to: '/docs/tutorial',
            },
            {
              label: 'Reference',
              to: '/docs/reference',
            },
          ],
        },
        {
          title: 'Community',
          items: [
            {
              label: 'Discord',
              href: 'https://discord.gg/thbTssDD7',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/boltlang',
            },
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'Blog',
              to: '/blog',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} The Bolt Programming Language Contributors. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;

import { defineUserConfig } from 'vuepress'
import { viteBundler } from '@vuepress/bundler-vite'
import { markdownMathPlugin } from '@vuepress/plugin-markdown-math'
import { registerComponentsPlugin } from '@vuepress/plugin-register-components'
import { defaultTheme } from '@vuepress/theme-default'
import { path } from '@vuepress/utils'

export default defineUserConfig({
  lang: 'en-GB',
  title: 'Optimist',
  description: 'Simulate your system design before you build it.',

  head: [
    [
      'meta',
      {
        name: 'description',
        content:
          'Documentation for Optimist, a system design tool which simulates your architecture, models its behaviour under load, and tells you what constrains it.',
      },
    ],
    ['link', { rel: 'icon', href: 'https://cdn.sierrasoftworks.com/logos/icon.png' }],
  ],

  bundler: viteBundler(),

  plugins: [
    markdownMathPlugin({
      type: 'katex',
      delimiters: 'dollars',
    }),
    registerComponentsPlugin({
      componentsDir: path.resolve(__dirname, './components'),
    }),
  ],

  theme: defaultTheme({
    logo: 'https://cdn.sierrasoftworks.com/logos/icon.png',
    logoDark: 'https://cdn.sierrasoftworks.com/logos/icon_light.png',
    repo: 'SierraSoftworks/optimist',
    docsRepo: 'SierraSoftworks/optimist',
    docsDir: 'docs',
    editLink: true,
    lastUpdated: true,

    navbar: [
      { text: 'Getting Started', link: '/guide/' },
      {
        text: 'Guides',
        children: [
          '/guide/modelling.md',
          '/guide/nalsd.md',
          '/guide/component-types.md',
          '/guide/language.md',
          '/guide/distributions.md',
          '/guide/uncertainty.md',
          '/guide/analysis.md',
          '/guide/collaboration.md',
        ],
      },
      {
        text: 'Reference',
        children: [
          '/reference/cli.md',
          '/reference/http-api.md',
          '/reference/yaml.md',
          '/reference/catalogue.md',
          '/reference/laws.md',
        ],
      },
      { text: 'Examples', link: '/examples/' },
      {
        text: 'Download',
        link: 'https://github.com/SierraSoftworks/optimist/releases',
        target: '_blank',
      },
      {
        text: 'Report an Issue',
        link: 'https://github.com/SierraSoftworks/optimist/issues/new',
        target: '_blank',
      },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Using Optimist',
          children: [
            '/guide/README.md',
            '/guide/modelling.md',
            '/guide/nalsd.md',
            '/guide/component-types.md',
            '/guide/collaboration.md',
          ],
        },
        {
          text: 'Writing expressions',
          children: [
            '/guide/language.md',
            '/guide/distributions.md',
            '/guide/uncertainty.md',
          ],
        },
        {
          text: 'Getting answers',
          children: ['/guide/analysis.md'],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          children: [
            '/reference/README.md',
            '/reference/cli.md',
            '/reference/http-api.md',
            '/reference/yaml.md',
            '/reference/catalogue.md',
            '/reference/laws.md',
          ],
        },
      ],
      '/examples/': [
        {
          text: 'Examples',
          children: ['/examples/README.md'],
        },
      ],
    },
  }),
})

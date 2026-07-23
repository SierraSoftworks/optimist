import { defineUserConfig } from 'vuepress'
import { viteBundler } from '@vuepress/bundler-vite'
import { markdownMathPlugin } from '@vuepress/plugin-markdown-math'
import { defaultTheme } from '@vuepress/theme-default'

export default defineUserConfig({
  lang: 'en-GB',
  title: 'Optimist',
  description: 'Model complex systems, uncertainty, feedback loops, and investment decisions.',

  head: [
    [
      'meta',
      {
        name: 'description',
        content:
          'Documentation for Optimist, a typed causal modelling and uncertainty analysis toolkit.',
      },
    ],
  ],

  bundler: viteBundler(),

  plugins: [
    markdownMathPlugin({
      type: 'katex',
      delimiters: 'dollars',
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
          '/guide/uncertainty.md',
          '/guide/analysis.md',
          '/guide/collaboration.md',
        ],
      },
      {
        text: 'Reference',
        children: ['/reference/cli.md', '/reference/http-api.md', '/reference/yaml.md'],
      },
      { text: 'Examples', link: '/examples/' },
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
            '/guide/uncertainty.md',
            '/guide/analysis.md',
            '/guide/collaboration.md',
          ],
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

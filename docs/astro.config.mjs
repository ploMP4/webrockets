// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
    integrations: [
        starlight({
            title: 'webrockets',
            description: 'High-performance WebSocket server for Python with Django support',
            social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/ploMP4/webrockets' }],
            sidebar: [
                {
                    label: 'Getting Started',
                    items: [
                        { label: 'Introduction', slug: 'guides/introduction' },
                        { label: 'Quick Start', slug: 'guides/quick-start' },
                    ],
                },
                {
                    label: 'Guides',
                    items: [
                        { label: 'Authentication', slug: 'guides/authentication' },
                        { label: 'Pattern Matching', slug: 'guides/pattern-matching' },
                        { label: 'Broadcasting', slug: 'guides/broadcasting' },
                        { label: 'Logging', slug: 'guides/logging' },
                    ],
                },
                {
                    label: 'Django',
                    items: [
                        { label: 'Setup', slug: 'django/setup' },
                        { label: 'Authentication', slug: 'django/authentication' },
                        { label: 'Broadcasting', slug: 'django/broadcasting' },
                        {
                            label: 'Deployment',
                            items: [
                                { label: 'Overview', slug: 'django/deployment/overview' },
                                { label: 'Nginx', slug: 'django/deployment/nginx' },
                                { label: 'Traefik', slug: 'django/deployment/traefik' },
                            ],
                        },
                    ],
                },
                {
                    label: 'Reference',
                    items: [
                        { label: 'WebsocketServer', slug: 'reference/server' },
                        { label: 'WebsocketRoute', slug: 'reference/route' },
                        { label: 'Connection', slug: 'reference/connection' },
                        { label: 'Match', slug: 'reference/match' },
                        { label: 'Authentication', slug: 'reference/authentication' },
                        { label: 'Broadcasting', slug: 'reference/broadcasting' },
                    ],
                },
            ],
        }),
    ],
});

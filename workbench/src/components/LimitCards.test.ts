import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import type { Bottleneck, Movement } from '../api/types'
import LimitCards from './LimitCards.vue'

const bottleneck: Bottleneck = {
  component: 'browsers',
  constraint: 'throughput',
  summary: 'Demand against capacity.',
  replicas: 1,
  utilisation: 1.14,
  utilisation_p90: 1.2,
  probability_of_binding: 1,
  headroom: -14,
}

const movement: Movement = {
  component: 'browsers',
  constraint: 'throughput',
  before: 0.44,
  after: 1.14,
  bound_before: 0,
  bound_after: 1,
}

it('shows ordinary constraints as ratios and absolute ratio shifts', () => {
  const wrapper = mount(LimitCards, {
    props: {
      bottlenecks: [bottleneck],
      movements: { 'browsers/throughput': movement },
    },
  })

  expect(wrapper.get('.load').text()).toBe('1.14x')
  expect(wrapper.get('.shift').text()).toBe('+0.7x')
  expect(wrapper.get('.fill').attributes('style')).toContain('width: 100%')
  expect(wrapper.get('.limit').classes()).toContain('binding')
})

it.each([
  { current: 0.995, objective: 0.99, tone: 'healthy' },
  { current: 0.985, objective: 0.99, tone: 'binding' },
  { current: 0.995, objective: undefined, tone: 'binding' },
])('shows SLI performance and colours it against its objective', ({ current, objective, tone }) => {
  const entry = { ...bottleneck, constraint: 'success_objective' }
  const wrapper = mount(LimitCards, {
    props: {
      bottlenecks: [entry],
      serviceLevels: {
        'browsers/success_objective': { current, baseline: 1, objective },
      },
    },
  })

  expect(wrapper.get('.load').text()).toBe(`${current * 100}%`)
  expect(wrapper.get('.shift').text()).toBe(`\u2212${Number(((1 - current) * 100).toFixed(1))}%`)
  expect(wrapper.get('.limit').classes()).toContain(tone)
})

it('waits for the SLI baseline before showing its shift', () => {
  const entry = { ...bottleneck, constraint: 'success_objective' }
  const wrapper = mount(LimitCards, {
    props: {
      bottlenecks: [entry],
      movements: { 'browsers/success_objective': movement },
      serviceLevels: {
        'browsers/success_objective': { current: 0.995, objective: 0.99 },
      },
    },
  })

  expect(wrapper.find('.shift').exists()).toBe(false)
})
it('keeps a constraint the variant relieved, however far it has fallen', () => {
  const pressing: Bottleneck = { ...bottleneck, constraint: 'concurrency', utilisation: 2 }
  const relieved: Bottleneck = { ...bottleneck, utilisation: 0.01, probability_of_binding: 0, headroom: 99 }
  const wrapper = mount(LimitCards, {
    props: {
      bottlenecks: [pressing, relieved],
      movements: {
        'browsers/throughput': { ...movement, before: 1.14, after: 0.01, bound_before: 1, bound_after: 0 },
      },
    },
  })

  const card = wrapper.get('[data-test="limit-browsers-throughput"]')
  expect(card.get('.note').text()).toBe('no longer binds')
  expect(card.get('.shift').text()).toBe('\u22121.13x')
})

it('shows a relieved constraint even when more pressing ones fill the cards', () => {
  const pressing = [2, 1.9, 1.8, 1.7].map((utilisation, index) => ({
    ...bottleneck,
    constraint: `pressing_${index}`,
    utilisation,
  }))
  const relieved: Bottleneck = { ...bottleneck, utilisation: 0.01, probability_of_binding: 0, headroom: 99 }
  const wrapper = mount(LimitCards, {
    props: {
      bottlenecks: [...pressing, relieved],
      movements: {
        'browsers/throughput': { ...movement, before: 1.14, after: 0.01, bound_before: 1, bound_after: 0 },
      },
    },
  })

  expect(wrapper.findAll('.limit')).toHaveLength(5)
  expect(wrapper.get('[data-test="limit-browsers-throughput"] .note').text()).toBe('no longer binds')
  expect(wrapper.find('.rest').exists()).toBe(false)
})

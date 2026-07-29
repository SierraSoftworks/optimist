import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import type { Frame } from '../api/types'
import MetricTimeline from './MetricTimeline.vue'

function series(value: number): Frame[] {
  return [
    {
      time: 0,
      converged: true,
      components: {
        api: {
          utilisation: { mean: value, p10: value, p50: value, p90: value, draws: [] },
        },
      },
    },
  ]
}

function mountPercentage(value: number) {
  return mount(MetricTimeline, {
    props: { series: series(value), component: 'api', channel: 'utilisation', unit: '%' },
  })
}

it('keeps percentages within their conventional range on a 0% to 100% axis', () => {
  const wrapper = mountPercentage(0.7)

  expect(wrapper.findAll('.tick').map((tick) => tick.text())).toEqual(['0%', '50%', '100%'])
})

it.each([
  { value: 1.2, edge: 'high', beyond: 100 },
  { value: -0.2, edge: 'low', beyond: 0 },
])('softens the $edge percentage bound for $value', ({ value, edge, beyond }) => {
  const wrapper = mountPercentage(value)
  const labels = wrapper.findAll('.tick').map((tick) => Number.parseFloat(tick.text()))
  const points = wrapper.get('polyline.average').attributes('points')
  expect(points).toBeDefined()
  const point = points!.split(',')
  const vertical = Number(point[1])

  expect(edge === 'high' ? labels.at(-1) : labels[0])[edge === 'high' ? 'toBeGreaterThan' : 'toBeLessThan'](
    beyond,
  )
  expect(vertical).toBeGreaterThanOrEqual(10)
  expect(vertical).toBeLessThanOrEqual(100)
})
import { describe, expect, it } from 'vitest'
import { commandShortcutLabel } from './platformShortcut'

describe('commandShortcutLabel', () => {
  it('uses Cmd on Apple platforms', () => {
    expect(commandShortcutLabel('MacIntel')).toBe('Cmd+K')
    expect(commandShortcutLabel('iPhone')).toBe('Cmd+K')
  })

  it('uses Ctrl on other and unknown platforms', () => {
    expect(commandShortcutLabel('Win32')).toBe('Ctrl+K')
    expect(commandShortcutLabel('Linux x86_64')).toBe('Ctrl+K')
    expect(commandShortcutLabel('')).toBe('Ctrl+K')
  })
})
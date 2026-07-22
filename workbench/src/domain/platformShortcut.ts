export function commandShortcutLabel(platform = browserPlatform()) {
  return /Mac|iPhone|iPad|iPod/i.test(platform) ? 'Cmd+K' : 'Ctrl+K'
}

function browserPlatform() {
  return typeof navigator === 'undefined' ? '' : navigator.platform
}
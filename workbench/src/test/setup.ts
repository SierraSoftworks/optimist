class ResizeObserverMock {
	observe() {}

	unobserve() {}

	disconnect() {}
}

Object.defineProperty(globalThis, 'ResizeObserver', {
	configurable: true,
	value: ResizeObserverMock,
})
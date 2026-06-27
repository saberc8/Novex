import { afterEach, beforeEach, vi } from "vitest";

class ResizeObserverMock {
  private readonly callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
  }

  observe(target: Element) {
    queueMicrotask(() => {
      this.callback(
        [
          {
            target,
            contentRect: {
              x: 0,
              y: 0,
              width: 1180,
              height: 640,
              top: 0,
              left: 0,
              right: 1180,
              bottom: 640,
              toJSON: () => ({ width: 1180, height: 640 })
            }
          } as ResizeObserverEntry
        ],
        this as unknown as ResizeObserver
      );
    });
  }

  unobserve() {}

  disconnect() {}
}

class DOMMatrixReadOnlyMock {
  readonly m11: number;
  readonly m22: number;

  constructor(transform?: string) {
    const values = transform
      ?.match(/matrix\(([^)]+)\)/)?.[1]
      ?.split(",")
      .map((value) => Number(value.trim()));
    this.m11 = values?.[0] && Number.isFinite(values[0]) ? values[0] : 1;
    this.m22 = values?.[3] && Number.isFinite(values[3]) ? values[3] : this.m11;
  }
}

beforeEach(() => {
  Object.defineProperty(globalThis, "ResizeObserver", {
    configurable: true,
    writable: true,
    value: ResizeObserverMock
  });
  Object.defineProperty(globalThis, "DOMMatrixReadOnly", {
    configurable: true,
    writable: true,
    value: DOMMatrixReadOnlyMock
  });
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function getOffsetWidth(this: HTMLElement) {
    return this.classList.contains("react-flow__node") ? 220 : 1180;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function getOffsetHeight(this: HTMLElement) {
    return this.classList.contains("react-flow__node") ? 64 : 640;
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function getBoundingClientRect(this: HTMLElement) {
    const width = this.classList.contains("react-flow__node") ? 220 : 1180;
    const height = this.classList.contains("react-flow__node") ? 64 : 640;
    return {
      x: 0,
      y: 0,
      width,
      height,
      top: 0,
      left: 0,
      right: width,
      bottom: height,
      toJSON: () => ({ width, height })
    } as DOMRect;
  });
  Object.defineProperty(SVGElement.prototype, "getBBox", {
    configurable: true,
    value: () => ({
      x: 0,
      y: 0,
      width: 72,
      height: 16,
      top: 0,
      left: 0,
      right: 72,
      bottom: 16,
      toJSON: () => ({ width: 72, height: 16 })
    })
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  delete (SVGElement.prototype as SVGElement & { getBBox?: unknown }).getBBox;
});

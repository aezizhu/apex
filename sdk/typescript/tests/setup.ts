/**
 * Jest test setup file
 */

// Increase timeout for async operations
jest.setTimeout(10000);

// Mock console methods to reduce noise in tests
beforeEach(() => {
  jest.spyOn(console, 'log').mockImplementation(() => {});
  jest.spyOn(console, 'warn').mockImplementation(() => {});
  jest.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  jest.restoreAllMocks();
});

// Global test utilities
export const createMockResponse = <T>(data: T, status = 200) => ({
  data: { success: true, data },
  status,
  statusText: 'OK',
  headers: {},
  config: {} as never,
});

export const createMockErrorResponse = (
  status: number,
  error: { code: string; message: string; details?: Record<string, unknown> }
) => ({
  response: {
    data: { success: false, error },
    status,
    statusText: 'Error',
    headers: {},
    config: {} as never,
  },
});

export const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

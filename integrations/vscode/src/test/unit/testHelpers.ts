import {
  DisassemblyHeader,
  DisassemblyInstruction,
  DisassemblyFunction,
  DisassemblyResult,
} from '../../iplcRendering';
import { LanguageClientLike, STATE_RUNNING, STATE_STOPPED } from '../../iplcEditorLogic';

// Re-export state constants for test use.
export { STATE_RUNNING, STATE_STOPPED };

export function createMockClient(overrides?: Partial<LanguageClientLike>): LanguageClientLike {
  return {
    isRunning: () => true,
    sendRequest: () => Promise.resolve(createTestDisassemblyResult()),
    onDidChangeState: () => ({ dispose: () => {} }),
    ...overrides,
  };
}

export function createTestHeader(overrides?: Partial<DisassemblyHeader>): DisassemblyHeader {
  return {
    formatVersion: 1,
    flags: { hasContentSignature: false, hasDebugSection: false, hasTypeSection: false },
    maxStackDepth: 16,
    maxCallDepth: 4,
    numVariables: 2,
    numFbInstances: 0,
    numFunctions: 1,
    numFbTypes: 0,
    numArrays: 0,
    entryFunctionId: 0,
    inputImageBytes: 64,
    outputImageBytes: 64,
    memoryImageBytes: 128,
    contentHash: 'abc123',
    sourceHash: 'def456',
    ...overrides,
  };
}

export function createTestInstruction(overrides?: Partial<DisassemblyInstruction>): DisassemblyInstruction {
  return {
    offset: 0,
    opcode: 'LOAD',
    operands: '0',
    comment: '',
    ...overrides,
  };
}

export function createTestFunction(overrides?: Partial<DisassemblyFunction>): DisassemblyFunction {
  return {
    id: 0,
    maxStackDepth: 8,
    numLocals: 2,
    bytecodeLength: 16,
    instructions: [createTestInstruction()],
    ...overrides,
  };
}

export function createTestDisassemblyResult(overrides?: Partial<DisassemblyResult>): DisassemblyResult {
  return {
    header: createTestHeader(),
    constants: [],
    functions: [createTestFunction()],
    ...overrides,
  };
}

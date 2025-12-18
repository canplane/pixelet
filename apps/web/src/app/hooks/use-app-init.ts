import { useState, useEffect, useRef } from 'react';
import { useTimeout } from '@/shared/lib/hooks/use-timer';
import { useSyncLocation } from '@/entities/device-location/model/use-sync-location';

/**
 * Constants for initialization timing (in milliseconds)
 */
const TIME_IDLE: number = 100; // Buffer for initial handshake
const TIME_LOADING: number = 500; // Display loading screen after this delay
const TIME_TIMEOUT: number = 5000; // Trigger timeout after this duration

export const LOAD_STATE = {
  IDLE: 'idle', // Initial state
  LOADING: 'loading', // Show loading UI (after threshold)
  SUCCESS: 'success', // Initialization complete
  ERROR: 'error', // Initialization failed
  TIMEOUT: 'timeout', // Operation exceeded threshold
} as const;

/** Load State */
export type LoadState = (typeof LOAD_STATE)[keyof typeof LOAD_STATE];

/** AppInitResult */
type AppInitResult = {
  loadState: LoadState;
  store: unknown;
};

/**
 * Hook to manage the global application initialization process.
 */
export function useAppInit(): AppInitResult {
  const [loadState, setLoadState] = useState<LoadState>('success');

  useSyncLocation();

  // // Use a ref to access the latest state within timeouts without re-triggering them
  // const loadStateRef = useRef(loadState);

  // useEffect(() => {
  //   loadStateRef.current = loadState;
  // }, [loadState]);

  // /**
  //  * Transition to 'loading' state if initialization takes longer than LOADING_THRESHOLD
  //  */
  // useTimeout(() => {
  //   if (loadStateRef.current === 'idle') {
  //     setLoadState('loading');
  //   }
  // }, TIME_LOADING);

  // /**
  //  * Transition to 'timeout' state if initialization exceeds TIMEOUT_THRESHOLD
  //  */
  // useTimeout(() => {
  //   if (loadStateRef.current !== 'success') {
  //     setLoadState('timeout');
  //   }
  // }, TIME_TIMEOUT);

  // Initialize your global store or state management here
  const [store] = useState<{}>(() => {});

  // useTimeout(() => {}, TIME_IDLE);

  return { store, loadState };
}

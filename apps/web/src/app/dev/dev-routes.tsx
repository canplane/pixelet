import { Outlet } from 'react-router-dom';
import type { RouteObject } from 'react-router-dom';

import TestPage from '@/pages/dev/test-page';

/** Debug mode only accessible routes */
export const devRoutes: RouteObject[] = [
  {
    path: 'dev',
    element: <Outlet />,
    children: [{ path: 'test', element: <TestPage /> }],
  },
];

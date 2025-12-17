import { Outlet } from 'react-router-dom';
import type { RouteObject } from 'react-router-dom';

/** Debug mode only accessible routes */
export const devRoutes: RouteObject[] = [
  {
    path: 'dev',
    element: <Outlet />,
    children: [
      // { path: 'components', element: <ComponentLibrary /> }
    ],
  },
];

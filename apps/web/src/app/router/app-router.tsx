import {
  createBrowserRouter,
  Navigate,
  Outlet,
  RouterProvider,
} from 'react-router-dom';
import type { RouteObject } from 'react-router-dom';

import { ScrollToTop } from './helpers/scroll-to-top';
import { devRoutes } from '../dev/dev-routes';
import { AppLayout } from '../ui/app-layout';
import MapPage from '@/pages/map-page';

const IS_DEV = import.meta.env.DEV;

/** Main Router Configuration */
const routes: RouteObject[] = [
  {
    path: '/',
    element: (
      <>
        <ScrollToTop />

        <AppLayout>
          <Outlet />
        </AppLayout>
      </>
    ),
    children: [
      { path: '', element: <MapPage /> }, // Map page

      ...(IS_DEV ? devRoutes : []), // Debug mode only accessible routes

      { path: '*', element: <Navigate to="/" replace /> },
    ],
  },
];

export const router = createBrowserRouter(routes);

export function AppRouter() {
  return <RouterProvider router={router} />;
}

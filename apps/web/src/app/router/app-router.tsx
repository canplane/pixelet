import {
  createBrowserRouter,
  Navigate,
  Outlet,
  RouterProvider,
} from 'react-router-dom';
import type { RouteObject } from 'react-router-dom';

import { AppLayout } from '@/app/layouts/app-layout';
import { ScrollToTop } from '@/app/router/helpers/scroll-to-top';
import IndexPage from '@/pages/index-page';
import { devRoutes } from '@/app/dev/dev-routes';

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
      { path: '', element: <IndexPage /> }, // Index page

      ...(IS_DEV ? devRoutes : []), // Debug mode only accessible routes

      { path: '*', element: <Navigate to="/" replace /> },
    ],
  },
];

export const router = createBrowserRouter(routes);

export function AppRouter() {
  return <RouterProvider router={router} />;
}

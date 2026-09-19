// @pxl/web/app/router.tsx

import { createBrowserRouter, Navigate, Outlet, RouterProvider } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import { ScrollToTop } from "./scroll-to-top.tsx";
import { devRoutes } from "@/feature/dev/dev-route.tsx";
import { AppLayout } from "./layout.tsx";
import MapPage from "@/page/map-page.tsx";

/** Main Router Configuration */
const routes: RouteObject[] = [
  {
    path: "/",
    element: (
      <>
        <ScrollToTop />

        <AppLayout>
          <Outlet />
        </AppLayout>
      </>
    ),
    children: [
      { path: "", element: <MapPage /> }, // Map page

      ...(__IS_DEV__ ? devRoutes : []), // Debug mode only accessible routes

      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
];

export const router = createBrowserRouter(routes);

export function AppRouter() {
  return <RouterProvider router={router} />;
}

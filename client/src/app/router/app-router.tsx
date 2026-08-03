// @pxl/web/app/router/app-router.tsx

import { createBrowserRouter, Navigate, Outlet, RouterProvider } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import { ScrollToTop } from "./helpers/scroll-to-top.tsx";
import { devRoutes } from "../dev/dev-routes.tsx";
import { AppLayout } from "../ui/app-layout.tsx";
import MapPage from "@/pages/map-page.tsx";

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

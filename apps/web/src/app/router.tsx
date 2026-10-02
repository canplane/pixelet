// @pxl/web/app/router.tsx

import { createBrowserRouter, Navigate, Outlet, RouterProvider } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import { devRoutes } from "@/feature/dev/dev-route.tsx";
import { AppLayout } from "./layout.tsx";
import MapPage from "@/page/map-page.tsx";

const routes: RouteObject[] = [
  {
    path: "/",
    element: (
      <AppLayout>
        <Outlet />
      </AppLayout>
    ),
    children: [
      { path: "", element: <MapPage /> },

      ...(__IS_DEV__ ? devRoutes : []),

      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
];

export const router = createBrowserRouter(routes);

export function AppRouter() {
  return <RouterProvider router={router} />;
}

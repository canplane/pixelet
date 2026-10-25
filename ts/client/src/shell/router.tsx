// @pxl/client/shell/router.tsx

import { createBrowserRouter, Navigate, Outlet, RouterProvider } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import { devRoutes } from "@/shell/dev/route.tsx";
import { AppLayout } from "./layout.tsx";
import WorldPage from "@/shell/world/page.tsx";

const routes: RouteObject[] = [
  {
    path: "/",
    element: (
      <AppLayout>
        <Outlet />
      </AppLayout>
    ),
    children: [
      { path: "", element: <WorldPage /> },

      ...(__IS_DEV__ ? devRoutes : []),

      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
];

const router = createBrowserRouter(routes);

export function AppRouter() {
  return <RouterProvider router={router} />;
}

// @pxl/web/app/dev/dev_routes.tsx

import { Outlet } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import TestPage from "@/pages/dev/test_page.tsx";

/** Debug mode only accessible routes */
export const devRoutes: RouteObject[] = [
  {
    path: "dev",
    element: <Outlet />,
    children: [{ path: "test", element: <TestPage /> }],
  },
];

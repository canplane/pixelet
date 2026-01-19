// @pxl/web/modules/map/ui/toast/toast-controller.tsx

import { useEffect } from "react";
import { toast } from "sonner";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";

import { DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { MapDataStatus, useMapStore } from "@/modules/map/navigation/map-store.ts";

import { DynamicIslandLayout } from "@/shared/ui/toast/dynamic-island-layout.tsx";

export function MapToastController() {
  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  const location_error_msg: string | null = useDeviceLocationStore((state) => state.error_msg);

  const map_status: MapDataStatus = useMapStore((state) => state.status);

  function get_toast_cfg() {
    // 공통적으로 쓰이는 로딩 아이콘
    const LoadingIcon = (
      <motion.div
        key="loading-icon"
        initial={{ opacity: 0, scale: 0.8 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.5 }}
        transition={{ duration: 0.2 }}
      >
        <Loader2 className="w-4 h-4 animate-spin-slow stroke-[2.5px]" />
      </motion.div>
    );

    // 공통적으로 쓰이는 성공 아이콘
    const SuccessIcon = (
      <motion.div
        key="success-icon"
        initial={{ opacity: 0, scale: 0.3, rotate: -20 }}
        animate={{ opacity: 1, scale: 1, rotate: 0 }}
        transition={{ type: "spring", damping: 12, stiffness: 260, mass: 0.8 }}
      >
        <CheckCircle2 className="w-4 h-4 text-[#34C759] stroke-[2.5px]" />
      </motion.div>
    );

    const isError = location_status === DevicePoseStatus.ERROR || map_status === MapDataStatus.ERROR;
    if (isError) {
      return {
        id: "location-error", // 에러는 별도 ID 유지 (교체되지 않도록)
        mode: "error" as const,
        title: location_error_msg || "GPS Signal Weak",
        description: "Signal",
        subDescription: "실외로 이동하거나 잠시만 기다려주세요.",
        statusColor: "bg-red-600",
        icon: <MapPinOff className="w-4 h-4 text-red-500" />,
        duration: 5000,
        // 원래 디자인의 붉은색 유리 느낌을 backgroundColor와 아이콘 색상으로 계승
        backgroundColor: "bg-[#1A0505]/90 backdrop-blur-2xl",
        className: "border-red-900/30 shadow-[0_10px_40px_rgba(220,38,38,0.2)]",
      };
    }

    if (location_status === DevicePoseStatus.LOADING) {
      return {
        id: "map-status-island",
        mode: "status" as const,
        status: "loading" as const,
        title: "당신의 위치를 찾는 중입니다",
        description: "MAP",
        subDescription: "정확한 위치를 확인하고 있어요.",
        statusColor: "bg-[#34C759]/30",
        icon: LoadingIcon,
        duration: Infinity,
      };
    }

    // 위치는 찾았으나 지도가 로딩 중
    if (location_status === DevicePoseStatus.READY && map_status === MapDataStatus.LOADING) {
      // 🚀 감성 매핑 로직
      const content = {
        roads: {
          title: "주변 공간을 불러오고 있어요",
          // subDescription: '먼저 발밑의 길을 찾고 있습니다...',
        },
        poi: {
          title: "주변 공간을 불러오고 있어요", // 타이틀 유지로 안정감 부여
          // subDescription: '이제 가까운 장소들을 채우는 중이에요.',
        },
        all: {
          title: "공간 구성을 마무리하고 있어요",
          // subDescription: '잠시 후 지도가 완성됩니다.',
        },
      };

      // const currentContent = content[loadingStep as keyof typeof content] || content.roads;

      // return {
      //   id: "map-status-island",
      //   mode: "status" as const,
      //   status: "loading" as const,
      //   title: currentContent.title,
      //   description: "MAP",
      //   subDescription: (currentContent as any).subDescription,
      //   statusColor: "bg-[#34C759]/30",
      //   icon: LoadingIcon,
      //   duration: Infinity,
      // };
    }

    if (location_status === DevicePoseStatus.READY && map_status === MapDataStatus.SUCCESS) {
      return {
        id: "map-status-island",
        mode: "status" as const,
        status: "success" as const,
        title: "공간 동기화 완료",
        description: "MAP",
        statusColor: "bg-[#34C759]",
        icon: SuccessIcon,
        duration: 2000,
      };
    }

    return null;
  }

  useEffect(() => {
    const config = get_toast_cfg();

    // 🚀 해결 4: 상태가 유효하지 않은 초기값(예: idle 등)일 때는 토스트를 생성하지 않음
    if (!config) {
      toast.dismiss("map-status-island");
      return;
    }

    // Sonner의 toast.custom은 ID가 같으면 자동으로 부드럽게 내용만 교체합니다.
    toast.custom(
      () => (
        <DynamicIslandLayout
          title={config.title}
          description={config.description}
          subDescription={config.subDescription}
          statusColor={config.statusColor}
          icon={
            <div className="relative flex items-center justify-center w-4 h-4">
              <AnimatePresence mode="wait" initial={false}>
                {config.icon}
              </AnimatePresence>
            </div>
          }
          backgroundColor={config.backgroundColor}
          className={config.className}
        />
      ),
      { id: config.id, duration: config.duration },
    );
  }, [location_status, map_status, location_error_msg]);

  return null;
}

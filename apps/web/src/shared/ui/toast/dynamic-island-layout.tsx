// @pxl/web/shared/ui/toast/dynamic-island-layout.tsx

import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils/cn.ts";

type DynamicIslandLayoutProps = {
  children?: ReactNode;
  icon?: ReactNode;
  title: string;
  description?: string;
  subDescription?: string;
  statusColor?: string;
  className?: string;
  backgroundColor?: string;
};

export function DynamicIslandLayout({
  icon,
  title,
  description,
  subDescription,
  statusColor,
  className,
  backgroundColor = "bg-[#0A0A0A]",
}: DynamicIslandLayoutProps) {
  const isExpanded = !!(title || subDescription);
  const hasSub = !!subDescription;

  const springConfig = useMemo(
    () => ({
      type: "spring" as const,
      stiffness: 500,
      damping: 30, // 12에서 30으로 조정: auto 너비 대응 시 너무 튀면 레이아웃이 깨질 수 있음
      mass: 0.5,
    }),
    [],
  );

  const [wasExpanded, setWasExpanded] = useState(false);

  useEffect(() => {
    if (isExpanded) {
      const timer = setTimeout(() => setWasExpanded(true), 450);
      return () => clearTimeout(timer);
    } else {
      setWasExpanded(false);
    }
  }, [isExpanded]);

  // 1. 최대 높이를 미리 정의합니다. (hasSub일 때 64px, 아닐 때 44px 등)
  // 혹은 단순히 가장 큰 높이인 80px 정도로 고정해도 됩니다.
  const containerHeight = 80;

  return (
    /* 🚀 래퍼: Sonner가 보는 실제 '토스트'의 크기입니다.
       이 크기를 고정(h-[80px])하면 Sonner는 아일랜드가 커지든 말든 가만히 있습니다. */
    <div
      style={{ height: `${containerHeight}px` }}
      className="flex items-end justify-center w-full pointer-events-none"
    >
      <motion.div
        layout // 🚀 핵심: 레이아웃 변화를 자동으로 감지하여 애니메이션화함
        style={{ borderRadius: "999px" }} // 원형 유지
        transition={springConfig}
        className={cn(
          "relative inline-flex items-center justify-start shadow-2xl overflow-hidden pointer-events-auto shrink-0",
          "min-w-[44px] origin-bottom",
          backgroundColor,
          className,
        )}
      >
        {/* 1. 왼쪽 아이콘 박스 */}
        <motion.div
          layout // 내부 요소도 layout을 주어 부모의 크기 변화에 맞춰 위치 이동
          className={cn(
            "flex items-center justify-center shrink-0 z-20",
            isExpanded && wasExpanded && hasSub ? "w-[64px] h-[64px]" : "w-[44px] h-[44px]",
          )}
        >
          <div
            className={cn(
              "w-7 h-7 rounded-full bg-white/10 flex items-center justify-center text-white transition-transform duration-500",
              wasExpanded && hasSub ? "scale-125" : "scale-100",
            )}
          >
            {icon}
          </div>
        </motion.div>

        {/* 2. 가변 컨텐츠 컨테이너 */}
        <AnimatePresence>
          {isExpanded && wasExpanded && (
            <motion.div
              layout
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
              className="flex items-center h-full overflow-hidden shrink-0 whitespace-nowrap pr-4"
            >
              <div className="flex flex-col justify-center py-1">
                <div className="flex items-baseline gap-2 leading-none">
                  <span className="text-[13px] font-semibold text-white tracking-tight">
                    {title}
                  </span>
                  {description && (
                    <span className="text-[9px] font-black text-white/20 uppercase tracking-widest">
                      {description}
                    </span>
                  )}
                </div>
                {subDescription && (
                  <div className="text-[10px] font-medium text-white/40 mt-0.5 italic tracking-tight">
                    {subDescription}
                  </div>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* 3. 오른쪽 상태 점 */}
        <AnimatePresence>
          {isExpanded && wasExpanded && statusColor && (
            <motion.div
              layout
              initial={{ opacity: 0, scale: 0 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0 }}
              className="flex items-center justify-center pr-4 z-20"
            >
              <div className={cn("w-1.5 h-1.5 rounded-full", statusColor)} />
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

// return (
//   /* 🚀 래퍼: Sonner가 보는 실제 '토스트'의 크기입니다.
//      이 크기를 고정(h-[80px])하면 Sonner는 아일랜드가 커지든 말든 가만히 있습니다. */
//   <div
//     style={{ height: `${containerHeight}px` }}
//     className="flex items-end justify-center w-full pointer-events-none"
//   >
//     <motion.div
//       layout
//       // 🚀 최적화 2: transform-gpu를 강제하여 GPU 가속 활성화
//       // will-change는 꼭 필요한 속성만 타이트하게 유지
//       style={{ willChange: 'width, height', transform: 'translateZ(0)' }}
//       initial={{ width: 44, height: 44 }}
//       animate={{
//         width: isExpanded ? (wasExpanded ? 'auto' : [44, 44, 'auto']) : 44,
//         height: isExpanded
//           ? wasExpanded
//             ? expandedHeight
//             : [44, 44, expandedHeight]
//           : 44,
//       }}
//       transition={{
//         duration: isExpanded && !wasExpanded ? 0.8 : 0.4,
//         times: [0, 0.5, 1],
//         layout: springConfig,
//       }}
//       // 🚀 최적화 3: backface-visibility로 깜빡임(Flicker) 방지
//       className={cn(
//         'rounded-full relative inline-flex items-center justify-start shadow-2xl overflow-hidden pointer-events-auto shrink-0',
//         'min-w-[44px] backface-hidden origin-bottom',
//         backgroundColor,
//         // 'ring-1 ring-inset ring-white/10',
//         className,
//       )}
//     >
//       {/* 1. 왼쪽 아이콘 박스 */}
//       <motion.div
//         layout="position"
//         animate={{
//           width: isExpanded ? (wasExpanded ? expandedHeight : 44) : 44,
//           height: isExpanded ? (wasExpanded ? expandedHeight : 44) : 44,
//         }}
//         transition={springConfig}
//         className="flex items-center justify-center shrink-0 z-20"
//       >
//         <div
//           className={cn(
//             'w-7 h-7 rounded-full bg-white/10 flex items-center justify-center text-white transition-transform duration-500 will-change-transform',
//             wasExpanded && hasSub ? 'scale-140' : 'scale-100',
//           )}
//         >
//           {icon}
//         </div>
//       </motion.div>

//       {/* 2. 가변 컨텐츠 컨테이너 */}
//       <AnimatePresence mode="wait">
//         {isExpanded && (
//           <motion.div
//             key="content-wrapper"
//             layout="position"
//             initial={{ opacity: 0, width: 0 }}
//             animate={{
//               opacity: wasExpanded ? 1 : 0,
//               width: wasExpanded ? 'auto' : 0,
//             }}
//             exit={{ opacity: 0, width: 0 }}
//             transition={{
//               layout: springConfig,
//               opacity: { duration: 0.15 }, // 살짝 더 빠르게
//             }}
//             className="flex items-center h-full overflow-hidden shrink-0 whitespace-nowrap"
//           >
//             <div className="flex flex-col justify-center py-1 pl-1 pr-2">
//               <div className="flex items-baseline gap-2 leading-none">
//                 <span className="text-[13px] font-semibold text-white shrink-0 tracking-tight">
//                   {title}
//                 </span>
//                 {description && (
//                   <span className="text-[9px] font-black text-white/20 uppercase tracking-[0.1em] shrink-0">
//                     {description}
//                   </span>
//                 )}
//               </div>
//               {subDescription && (
//                 <div className="text-[10px] font-medium text-white/40 mt-0.5 italic shrink-0 tracking-tight">
//                   {subDescription}
//                 </div>
//               )}
//             </div>
//             {statusColor && <div className="w-[44px] shrink-0" />}
//           </motion.div>
//         )}
//       </AnimatePresence>

//       {/* 3. 오른쪽 점 */}
//       <AnimatePresence>
//         {isExpanded && wasExpanded && statusColor && (
//           <motion.div
//             key="status-dot-sticky"
//             layout="position"
//             // initial={{ opacity: 0, scale: 0 }}
//             // animate={{ opacity: 1, scale: 1 }}
//             // exit={{ opacity: 0, scale: 0 }}
//             // y: 10(아래)에서 0(제자리)으로 올라오며 팅김
//             initial={{ opacity: 0, scale: 0.5, y: 10 }}
//             animate={{ opacity: 1, scale: 1, y: 0 }}
//             exit={{ opacity: 0, scale: 0.5, y: 10 }}
//             transition={springConfig}
//             className="absolute right-0 w-[44px] h-full flex items-center justify-center z-20"
//           >
//             <div className={cn('w-1.5 h-1.5 rounded-full', statusColor)} />
//           </motion.div>
//         )}
//       </AnimatePresence>
//     </motion.div>
//   </div>

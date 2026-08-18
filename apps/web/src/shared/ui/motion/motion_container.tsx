// @pxl/web/shared/ui/motion/motion_container.tsx

import { AnimatePresence, type HTMLMotionProps, motion } from "framer-motion";

export const MAP_UI_VARIANTS = {
  slideUp: {
    initial: { opacity: 0, y: 15 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, scale: 0.9, y: 10 },
  },
  compass: {
    initial: { opacity: 0, scale: 0.8, rotate: -45, y: 10 },
    animate: { opacity: 1, scale: 1, rotate: 0, y: 0 },
    exit: { opacity: 0, scale: 0.5, rotate: 45 },
  },
  drop: {
    initial: { opacity: 0, y: -15 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: -10 },
  },
  riseUp: {
    initial: { opacity: 0, y: 15 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: 10 },
  },
};

interface MotionContainerProps extends HTMLMotionProps<"div"> {
  show?: boolean;
  variant?: keyof typeof MAP_UI_VARIANTS;
  delay?: number;
  layout?: boolean;
}

export function MotionContainer({
  children,
  show = true,
  variant = "slideUp",
  delay = 0,
  layout = true,
  className,
  ...props
}: MotionContainerProps) {
  return (
    <AnimatePresence mode="wait">
      {show && (
        <motion.div
          layout={layout}
          initial={MAP_UI_VARIANTS[variant].initial}
          animate={MAP_UI_VARIANTS[variant].animate}
          exit={MAP_UI_VARIANTS[variant].exit}
          transition={{
            duration: 0.6,
            ease: [0.16, 1, 0.3, 1],
            delay,
          }}
          className={className}
          {...props}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/shared/lib/utils/cn';

/**
 * Define Button Styles using CVA
 * - base: Common styles for all buttons
 * - variants: Differing styles based on props
 * - defaultVariants: Fallback values if props are missing
 */
const buttonVariants = cva(
  'inline-flex justify-center items-center gap-1.5 font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed outline-none focus-visible:ring-2 focus-visible:ring-offset-2',
  {
    variants: {
      color: {
        gray: 'bg-white text-gray-800 border border-gray-200 enabled:hover:bg-gray-50 enabled:active:bg-gray-100 focus-visible:ring-gray-400',
        blue: 'bg-blue-500 text-white enabled:hover:bg-blue-600 enabled:active:bg-blue-700 focus-visible:ring-blue-400',
        green:
          'bg-green-500 text-white enabled:hover:bg-green-600 enabled:active:bg-green-700 focus-visible:ring-green-400',
      },
      size: {
        sm: 'h-9 px-2.5 text-sm rounded',
        md: 'h-10 px-5 text-sm rounded-md',
        lg: 'h-12 px-5 text-base rounded-lg',
      },
    },
    defaultVariants: {
      color: 'gray',
      size: 'md',
    },
  },
);

/** ButtonProps */
type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants> & {
    label: string;
    icon?: React.ReactNode;
  };

/**
 * Common Button Component
 */
export function Button(props: ButtonProps) {
  const {
    size = 'md',
    label,
    color,
    icon,
    className,
    disabled,
    ...args
  } = props;

  return (
    <button
      type="button"
      className={cn(buttonVariants({ color, size }), className)}
      disabled={disabled}
      {...args}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}

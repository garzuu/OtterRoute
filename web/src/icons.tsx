import type { ReactNode } from "react";
import type { SVGProps } from "react";

/** Icone a tratto, 24×24, colore del testo. Nessuna dipendenza. */
type P = SVGProps<SVGSVGElement>;

const svg = (children: ReactNode, p: P) => (
  <svg
    width={18}
    height={18}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.7}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    {...p}
  >
    {children}
  </svg>
);

export const IconOverview = (p: P) =>
  svg(
    <>
      <rect x="3" y="3" width="7" height="9" rx="1.5" />
      <rect x="14" y="3" width="7" height="5" rx="1.5" />
      <rect x="14" y="12" width="7" height="9" rx="1.5" />
      <rect x="3" y="16" width="7" height="5" rx="1.5" />
    </>,
    p,
  );

export const IconDomains = (p: P) =>
  svg(
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18" />
      <path d="M12 3c2.6 2.4 4 5.5 4 9s-1.4 6.6-4 9c-2.6-2.4-4-5.5-4-9s1.4-6.6 4-9z" />
    </>,
    p,
  );

export const IconBuckets = (p: P) =>
  svg(
    <>
      <ellipse cx="12" cy="5.5" rx="8" ry="2.5" />
      <path d="M4 5.5v6c0 1.4 3.6 2.5 8 2.5s8-1.1 8-2.5v-6" />
      <path d="M4 11.5v6c0 1.4 3.6 2.5 8 2.5s8-1.1 8-2.5v-6" />
    </>,
    p,
  );

export const IconRoutes = (p: P) =>
  svg(
    <>
      <path d="M4 8h14" />
      <path d="M14 4l4 4-4 4" />
      <path d="M20 16H6" />
      <path d="M10 12l-4 4 4 4" />
    </>,
    p,
  );

export const IconSettings = (p: P) =>
  svg(
    <>
      <path d="M4 7h9" />
      <path d="M17 7h3" />
      <circle cx="15" cy="7" r="2" />
      <path d="M4 17h3" />
      <path d="M11 17h9" />
      <circle cx="9" cy="17" r="2" />
    </>,
    p,
  );

export const IconCheck = (p: P) => svg(<path d="M5 12.5l4.5 4.5L19 7.5" />, { strokeWidth: 2.2, ...p });
export const IconX = (p: P) => svg(<path d="M6 6l12 12M18 6L6 18" />, { strokeWidth: 2, ...p });
export const IconMinus = (p: P) => svg(<path d="M6 12h12" />, { strokeWidth: 2.2, ...p });
export const IconChevron = (p: P) => svg(<path d="M9 6l6 6-6 6" />, p);
export const IconSortNone = (p: P) => svg(<path d="M8 9l4-4 4 4M8 15l4 4 4-4" />, p);
export const IconSortAsc = (p: P) => svg(<path d="M8 14l4-4 4 4" />, { strokeWidth: 2.2, ...p });
export const IconSortDesc = (p: P) => svg(<path d="M8 10l4 4 4-4" />, { strokeWidth: 2.2, ...p });
export const IconMenu = (p: P) => svg(<path d="M4 7h16M4 12h16M4 17h16" />, p);
export const IconBell = (p: P) =>
  svg(
    <>
      <path d="M6 9a6 6 0 0 1 12 0c0 6 2 7.5 2 7.5H4S6 15 6 9z" />
      <path d="M10 20a2 2 0 0 0 4 0" />
    </>,
    p,
  );
export const IconLogout = (p: P) =>
  svg(
    <>
      <path d="M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4" />
      <path d="M15 8l4 4-4 4" />
      <path d="M19 12H9" />
    </>,
    p,
  );
export const IconChevronDown = (p: P) => svg(<path d="M6 9l6 6 6-6" />, p);
export const IconUser = (p: P) =>
  svg(
    <>
      <circle cx="12" cy="8" r="4" />
      <path d="M4 20c1.2-3.6 4.2-5.5 8-5.5s6.8 1.9 8 5.5" />
    </>,
    p,
  );
export const IconLock = (p: P) =>
  svg(
    <>
      <rect x="5" y="11" width="14" height="9" rx="2" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </>,
    p,
  );
export const IconEye = (p: P) =>
  svg(
    <>
      <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" />
      <circle cx="12" cy="12" r="3" />
    </>,
    p,
  );
export const IconEyeOff = (p: P) =>
  svg(
    <>
      <path d="M3 3l18 18" />
      <path d="M10.6 5.1A10 10 0 0 1 12 5c6.4 0 10 7 10 7a17 17 0 0 1-3.2 4.1" />
      <path d="M6.6 6.6A16.6 16.6 0 0 0 2 12s3.6 7 10 7a9.7 9.7 0 0 0 4.2-1" />
      <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
    </>,
    p,
  );
export const IconUsers = (p: P) =>
  svg(
    <>
      <circle cx="9" cy="8" r="3.5" />
      <path d="M2.5 20c.8-3.4 3.4-5.2 6.5-5.2s5.7 1.8 6.5 5.2" />
      <path d="M16 4.6a3.5 3.5 0 0 1 0 6.8" />
      <path d="M18 14.9c1.9.6 3.1 2.2 3.5 4.6" />
    </>,
    p,
  );
export const IconShield = (p: P) =>
  svg(
    <>
      <path d="M12 3l7 3v5.5c0 4.4-2.9 8-7 9.5-4.1-1.5-7-5.1-7-9.5V6l7-3z" />
      <path d="M9 12l2.2 2.2L15 10.4" />
    </>,
    p,
  );

export const IconDiagnosis = (p: P) =>
  svg(
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="M20 20l-4.2-4.2" />
      <path d="M8.5 11.2l1.7 1.7 3.3-3.6" />
    </>,
    p,
  );

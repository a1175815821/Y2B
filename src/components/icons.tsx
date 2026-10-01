/** 统一线性图标（24px 网格，stroke 风格），与设计规范配套 */
import React from "react";

type P = { size?: number; className?: string };

const base = (size: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
});

export const IconDownload = ({ size = 18, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M12 3v12m0 0 4.5-4.5M12 15l-4.5-4.5" />
    <path d="M4 17v2a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-2" />
  </svg>
);

export const IconCollection = ({ size = 18, className }: P) => (
  <svg {...base(size)} className={className}>
    <rect x="3.5" y="3.5" width="7.5" height="7.5" rx="1.5" />
    <rect x="13" y="3.5" width="7.5" height="7.5" rx="1.5" />
    <rect x="3.5" y="13" width="7.5" height="7.5" rx="1.5" />
    <rect x="13" y="13" width="7.5" height="7.5" rx="1.5" />
  </svg>
);

export const IconKey = ({ size = 18, className }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="8" cy="15" r="4.2" />
    <path d="M11.2 11.8 20 3m-3.5 3.5L19 9m-5.5-.5L16 11" />
  </svg>
);

export const IconGear = ({ size = 18, className }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="12" cy="12" r="3.2" />
    <path d="M12 2.8v2.6m0 13.2v2.6M4.2 4.2l1.8 1.8m12 12 1.8 1.8M2.8 12h2.6m13.2 0h2.6M4.2 19.8 6 18m12-12 1.8-1.8" />
  </svg>
);

export const IconSearch = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m16 16 4.5 4.5" />
  </svg>
);

export const IconFolder = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M3.5 6.5a1.5 1.5 0 0 1 1.5-1.5h4l1.5 2h7a1.5 1.5 0 0 1 1.5 1.5v7a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 3.5 15.5z" />
  </svg>
);

export const IconRefresh = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M20 12a8 8 0 1 1-2.34-5.66M20 3.5V8h-4.5" />
  </svg>
);

export const IconPlay = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M7 4.8v14.4L19 12z" />
  </svg>
);

export const IconCheck = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="m4.5 12.5 5 5 10-11" />
  </svg>
);

export const IconTrash = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M4 7h16M9.5 7V4.8A.8.8 0 0 1 10.3 4h3.4a.8.8 0 0 1 .8.8V7M6.5 7l1 13h9l1-13" />
  </svg>
);

export const IconLink = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M10 14a4.5 4.5 0 0 0 6.4.4l3-3a4.5 4.5 0 0 0-6.4-6.4l-1.7 1.7M14 10a4.5 4.5 0 0 0-6.4-.4l-3 3a4.5 4.5 0 0 0 6.4 6.4l1.7-1.7" />
  </svg>
);

export const IconInfo = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 11v5m0-8.5v.01" />
  </svg>
);

export const IconFilm = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <rect x="3" y="4.5" width="18" height="15" rx="2" />
    <path d="M7.5 4.5v15M16.5 4.5v15M3 9.5h4.5M3 14.5h4.5M16.5 9.5H21M16.5 14.5H21" />
  </svg>
);

export const IconMusic = ({ size = 16, className }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="7" cy="18" r="3" />
    <circle cx="17.5" cy="15.5" r="3" />
    <path d="M10 18V5.5L20.5 4v11.5" />
  </svg>
);

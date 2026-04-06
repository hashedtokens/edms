import React from 'react';
import { getMethodColor } from '../../utils/getMethodColor';

export const MethodBadge = ({ method, size = 'md', showCount, count }) => {
  const sizeClasses = {
    sm: 'px-2 py-1 text-xs',
    md: 'px-3 py-1.5 text-sm',
    lg: 'px-4 py-2 text-base'
  };

  return (
    <div className="flex items-center gap-2">
      <span 
        className={`method-badge ${sizeClasses[size]} rounded font-bold text-black uppercase tracking-wider`}
        style={{ backgroundColor: getMethodColor(method) }}
      >
        {method}
      </span>
      {showCount && count !== undefined && (
        <span className="text-sm font-semibold text-gray-700">
          {count}
        </span>
      )}
    </div>
  );
};
import React from 'react';
import { formatBytes } from '../../utils/formatBytes';

export const RequestDetail = ({ 
  title, 
  data, 
  requestNum,
  size 
}) => {
  return (
    <div className="flex-1 flex flex-col border-b border-gray-200 last:border-b-0">
      <div className="p-4 border-b border-gray-200 bg-gray-50">
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-semibold text-gray-800">
            {title} {requestNum ? requestNum : ''}
          </h4>
          {size !== undefined && (
            <span className="text-xs bg-gray-200 px-2 py-1 rounded font-mono">
              {formatBytes(size)}
            </span>
          )}
        </div>
      </div>
      
      <div className="flex-1 overflow-hidden p-4">
        <textarea 
          className="w-full h-full p-3 bg-gray-50 border border-gray-300 rounded font-mono text-sm resize-none outline-none"
          value={data ? JSON.stringify(data, null, 2) : ''}
          readOnly
          placeholder="Select a request to view details..."
        />
      </div>
    </div>
  );
};
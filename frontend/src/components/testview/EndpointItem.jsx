import React from 'react';
import { getMethodColor } from '../../utils/getMethodColor';

export const EndpointItem = ({ endpoint, onClick }) => {
  return (
    <div 
      className="mb-3 p-4 bg-white rounded-lg border border-gray-300 cursor-pointer transition-all hover:bg-gray-50 hover:border-purple-500"
      onClick={onClick}
    >
      <div className="flex justify-between items-center mb-3">
        <span 
          className="px-3 py-1 rounded text-xs font-bold text-white uppercase"
          style={{ backgroundColor: getMethodColor(endpoint.method) }}
        >
          {endpoint.method}
        </span>
      </div>
      <div className="text-sm text-gray-800 font-medium mb-2 truncate hover:text-purple-600">
        {endpoint.path}
      </div>
      <div className="flex justify-between items-center pt-2 border-t border-gray-100">
        <span className="text-xs text-gray-500 italic">
          {endpoint.category} • {endpoint.status}
        </span>
        <span className="text-xs text-gray-500">
          {endpoint.responseTime}ms
        </span>
      </div>
      {endpoint.description && (
        <div className="text-xs text-gray-500 mt-2">
          {endpoint.description}
        </div>
      )}
    </div>
  );
};
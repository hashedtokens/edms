import React from 'react';

export const RequestList = ({
  requests,
  selectedRequests,
  selectedRequestNum,
  onToggleRequest,
  onRequestClick
}) => {
  return (
    <div className="p-4">
      <div className="flex justify-between items-center px-3 py-2 bg-gray-50 rounded mb-3">
        <span className="text-sm font-semibold text-gray-700">Requests</span>
        <span className="text-xs font-semibold text-gray-600 bg-gray-200 px-2 py-1 rounded">
          {requests.length}
        </span>
      </div>
      <div className="flex flex-col gap-2">
        {requests.map((req) => (
          <div 
            key={req.num}
            className={`flex items-center gap-3 p-3 bg-gray-50 border rounded cursor-pointer transition-all ${
              selectedRequestNum === req.num 
                ? 'bg-blue-50 border-blue-300' 
                : 'border-gray-200 hover:bg-gray-100'
            }`}
          >
            <input 
              type="checkbox" 
              className="cursor-pointer w-4 h-4"
              checked={selectedRequests.includes(req.num)}
              onChange={() => onToggleRequest(req.num)}
            />
            <span 
              className="text-sm text-gray-700 font-medium flex-1"
              onClick={() => onRequestClick(req.num)}
            >
              Request {req.num}
            </span>
            <span className="text-xs text-gray-500 font-mono">
              {Math.round(req.metadata?.size_bytes / 1024 * 100) / 100} KB
            </span>
          </div>
        ))}
      </div>
    </div>
  );
};
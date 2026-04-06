import React from 'react';
import { RequestList } from './RequestList';

export const RequestSidebar = ({
  currentEndpoint,
  selectedRequests,
  selectedRequestNum,
  onToggleRequest,
  onRequestClick,
  onUnselectAll
}) => {
  if (!currentEndpoint) {
    return (
      <div className="h-full bg-white border-l border-gray-200 flex flex-col">
        <div className="p-5 border-b border-gray-200 bg-gray-50">
          <h3 className="text-lg font-semibold text-gray-800">Requests</h3>
        </div>
        <div className="flex-1 flex items-center justify-center p-4">
          <p className="text-gray-500 text-center">Select an endpoint to view requests</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full bg-white border-l border-gray-200 flex flex-col">
      <div className="p-5 border-b border-gray-200 bg-gray-50">
        <h3 className="text-lg font-semibold text-gray-800">Requests</h3>
      </div>
      
      <div className="p-4 border-b border-gray-200">
        <button 
          className="w-full px-3 py-2 bg-white border border-gray-300 rounded text-gray-700 text-sm hover:bg-gray-50 disabled:opacity-50"
          onClick={onUnselectAll}
          disabled={selectedRequests.length === 0}
        >
          Unselect All
        </button>
      </div>
      
      <div className="px-4 py-3 border-b border-gray-200 text-center">
        <span className="text-xs font-semibold text-gray-600 bg-gray-200 px-3 py-1 rounded-full">
          {selectedRequests.length} / {currentEndpoint.requests.length} selected
        </span>
      </div>

      {/* Scrollable request list */}
      <div className="flex-1 overflow-y-auto">
        <RequestList
          requests={currentEndpoint.requests}
          selectedRequests={selectedRequests}
          selectedRequestNum={selectedRequestNum}
          onToggleRequest={onToggleRequest}
          onRequestClick={onRequestClick}
        />
      </div>
    </div>
  );
};
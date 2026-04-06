import React from 'react';
import { JsonEditor } from '../common/JsonEditor';

export const RequestResponsePanels = ({ request, response, onRequestChange }) => {
  return (
    <div className="grid grid-cols-2 gap-6">
      <JsonEditor
        title="Request (JSON Editor)"
        value={request}
        onChange={onRequestChange}
        height="288px"
        showActions={true}
      />
      
      <JsonEditor
        title="Response (JSON Editor)"
        value={response}
        readOnly={true}
        height="288px"
        showActions={true}
      />
    </div>
  );
};
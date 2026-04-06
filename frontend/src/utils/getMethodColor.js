export const getMethodColor = (method) => {
  const colors = {
    'POST': '#49cc90',
    'GET': '#61affe',
    'PUT': '#fca130',
    'DELETE': '#f93e3e',
    'PATCH': '#50e3c2'
  };
  return colors[method] || '#999';
};
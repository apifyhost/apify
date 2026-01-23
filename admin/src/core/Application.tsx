import { useEffect } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { Spin } from 'antd';
import { AdminLayout } from '@/components/Layout/AdminLayout';
import { ProtectedRoute } from '@/components/ProtectedRoute';
import { Login } from '@/pages/Login';
import { Dashboard } from '@/pages/Dashboard';
import { ApisPage } from '@/pages/Apis';
import { DataSourcesPage } from '@/pages/DataSources';
import { ListenersPage } from '@/pages/Listeners';
import { SchemasPage } from '@/pages/Schemas';
import { useAppDispatch, useAppSelector } from './store/hooks';
import { fetchInitialData } from './store/appSlice';

export const Application = () => {
  const dispatch = useAppDispatch();
  const { loading, initialized } = useAppSelector((state) => state.app);

  useEffect(() => {
    dispatch(fetchInitialData());
  }, [dispatch]);

  if (loading || !initialized) {
    return (
      <div
        style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          height: '100vh',
        }}
      >
        <Spin size="large" tip="Loading..." />
      </div>
    );
  }

  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <AdminLayout />
          </ProtectedRoute>
        }
      >
        <Route index element={<Navigate to="/dashboard" replace />} />
        <Route path="dashboard" element={<Dashboard />} />
        <Route path="apis" element={<ApisPage />} />
        <Route path="datasources" element={<DataSourcesPage />} />
        <Route path="listeners" element={<ListenersPage />} />
        <Route path="schemas" element={<SchemasPage />} />
      </Route>
    </Routes>
  );
};

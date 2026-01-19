import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Form, Input, Button, Card, message } from 'antd';
import { LockOutlined } from '@ant-design/icons';
import { useAppDispatch } from '@/core/store/hooks';
import { login } from '@/core/store/appSlice';

export const Login = () => {
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();
  const dispatch = useAppDispatch();

  const onFinish = async (values: { apiKey: string }) => {
    setLoading(true);
    try {
      await dispatch(login(values.apiKey)).unwrap();
      message.success('登录成功');
      navigate('/dashboard');
    } catch (error) {
      message.error('登录失败，请检查 API Key');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        height: '100vh',
        background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
      }}
    >
      <Card
        style={{
          width: 400,
          boxShadow: '0 4px 12px rgba(0, 0, 0, 0.15)',
        }}
      >
        <div style={{ textAlign: 'center', marginBottom: 24 }}>
          <h1 style={{ fontSize: 28, marginBottom: 8 }}>Apify Admin</h1>
          <p style={{ color: '#666', fontSize: 14 }}>欢迎登录管理后台</p>
        </div>
        <Form
          name="login"
          initialValues={{ remember: true }}
          onFinish={onFinish}
          size="large"
        >
          <Form.Item
            name="apiKey"
            rules={[{ required: true, message: '请输入 API Key' }]}
          >
            <Input
              prefix={<LockOutlined />}
              type="password"
              placeholder="请输入 API Key"
              autoComplete="off"
            />
          </Form.Item>

          <Form.Item>
            <Button type="primary" htmlType="submit" block loading={loading}>
              登录
            </Button>
          </Form.Item>
        </Form>

        <div style={{ textAlign: 'center', color: '#999', fontSize: 12, marginTop: 16 }}>
          <p>默认 API Key: UZY65Nakvsd3</p>
          <p>可在配置文件中修改</p>
        </div>
      </Card>
    </div>
  );
};

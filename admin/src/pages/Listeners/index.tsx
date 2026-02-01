import { useState } from 'react';
import {
  Button,
  Table,
  Space,
  Modal,
  Form,
  Input,
  InputNumber,
  message,
  Tag,
  Popconfirm,
} from 'antd';
import { PlusOutlined, DeleteOutlined, CheckCircleOutlined } from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import {
  useGetListenersQuery,
  useCreateListenerMutation,
  useDeleteListenerMutation,
  Listener,
} from '@/services/api';

export const ListenersPage = () => {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [form] = Form.useForm();
  const { data: listeners = [], isLoading } = useGetListenersQuery();
  const [createListener, { isLoading: isCreating }] = useCreateListenerMutation();
  const [deleteListener] = useDeleteListenerMutation();

  const handleCreate = () => {
    setIsModalOpen(true);
    form.resetFields();
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      // Add default protocol
      await createListener({ ...values, protocol: 'http' }).unwrap();
      message.success('监听器创建成功');
      setIsModalOpen(false);
      form.resetFields();
    } catch (error) {
      message.error('创建失败');
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteListener(id).unwrap();
      message.success('删除成功');
    } catch (error) {
      message.error('删除失败');
    }
  };

  const columns: ColumnsType<Listener> = [
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
      width: 150,
    },
    {
      title: '主机',
      dataIndex: 'ip',
      key: 'ip',
      width: 150,
    },
    {
      title: '端口',
      dataIndex: 'port',
      key: 'port',
      width: 100,
    },
    {
      title: 'API 数量',
      key: 'api_count',
      width: 120,
      render: (_, record) => record.api_configs?.length || 0,
    },
    {
      title: '状态',
      key: 'status',
      width: 120,
      render: () => (
        <Tag icon={<CheckCircleOutlined />} color="success">
          运行中
        </Tag>
      ),
    },
    {
      title: '操作',
      key: 'action',
      width: 100,
      fixed: 'right',
      render: (_, record) => (
        <Space size="middle">
          <Popconfirm
            title="确定要删除这个监听器吗？"
            onConfirm={() => handleDelete(record.id)}
            okText="确定"
            cancelText="取消"
          >
            <Button type="link" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between' }}>
        <h1>监听器管理</h1>
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
          新建监听器
        </Button>
      </div>

      <Table
        columns={columns}
        dataSource={listeners}
        rowKey="id"
        loading={isLoading}
        scroll={{ x: 800 }}
        pagination={{
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条`,
        }}
      />

      <Modal
        title="创建监听器"
        open={isModalOpen}
        onOk={handleSubmit}
        onCancel={() => setIsModalOpen(false)}
        confirmLoading={isCreating}
        width={600}
      >
        <Form form={form} layout="vertical" style={{ marginTop: 24 }}>
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: '请输入监听器名称' }]}
          >
            <Input placeholder="例如: main-listener" />
          </Form.Item>
          <Form.Item
            name="ip"
            label="主机"
            rules={[{ required: true, message: '请输入主机地址' }]}
            initialValue="127.0.0.1"
          >
            <Input placeholder="例如: 127.0.0.1" />
          </Form.Item>
          <Form.Item
            name="port"
            label="端口"
            rules={[{ required: true, message: '请输入端口号' }]}
            initialValue={8080}
          >
            <InputNumber min={1} max={65535} style={{ width: '100%' }} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
};
